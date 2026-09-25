use crate::quoting::quote_windows_arg;
use service_kit_core::{RestartPolicy, Scope, ServiceSpec, is_windows_absolute_path};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs::OpenOptions,
    io, mem,
    os::windows::io::AsRawHandle,
    path::PathBuf,
    ptr,
    sync::{OnceLock, mpsc},
    time::{Duration, Instant},
};
use windows_service::{
    define_windows_service,
    service::{
        ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus,
        ServiceType,
    },
    service_control_handler::{self, ServiceControlHandlerResult, ServiceStatusHandle},
    service_dispatcher,
};
use windows_sys::Win32::{
    Foundation::{CloseHandle, HANDLE, HANDLE_FLAG_INHERIT, SetHandleInformation, WAIT_OBJECT_0},
    System::{JobObjects::*, Threading::*},
};
static SPEC: OnceLock<ServiceSpec> = OnceLock::new();
define_windows_service!(ffi_service_main, service_main);
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let path = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("expected protected spec.json path")?,
    );
    let expected = std::env::current_exe()?.with_file_name("spec.json");
    if path != expected {
        return Err("host only loads spec.json beside its protected executable".into());
    }
    let s: ServiceSpec = serde_json::from_slice(&std::fs::read(path)?)?;
    s.validate()?;
    if s.scope != Scope::System
        || s.restart == RestartPolicy::Always
        || !is_windows_absolute_path(&s.executable)
    {
        return Err("unsupported Windows scope, restart or executable".into());
    }
    let name = format!("service-kit-{}", s.id.as_str());
    SPEC.set(s).map_err(|_| "already initialized")?;
    service_dispatcher::start(name, ffi_service_main)?;
    Ok(())
}
fn report(
    h: &ServiceStatusHandle,
    state: ServiceState,
    code: ServiceExitCode,
    checkpoint: u32,
    wait: Duration,
) -> windows_service::Result<()> {
    h.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: state,
        controls_accepted: if state == ServiceState::Running {
            ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN
        } else {
            ServiceControlAccept::empty()
        },
        exit_code: code,
        checkpoint,
        wait_hint: wait,
        process_id: None,
    })
}
fn service_main(_: Vec<OsString>) {
    let s = SPEC.get().expect("initialized before dispatcher");
    let (tx, rx) = mpsc::channel();
    let h = match service_control_handler::register(
        format!("service-kit-{}", s.id.as_str()),
        move |control| match control {
            ServiceControl::Stop | ServiceControl::Shutdown => {
                let _ = tx.send(());
                ServiceControlHandlerResult::NoError
            }
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            _ => ServiceControlHandlerResult::NotImplemented,
        },
    ) {
        Ok(h) => h,
        Err(_) => return,
    };
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        report(
            &h,
            ServiceState::StartPending,
            ServiceExitCode::Win32(0),
            1,
            Duration::from_secs(15),
        )?;
        supervise(s, &h, rx)
    })();
    let code = if let Err(e) = result {
        use std::io::Write;
        if let Ok(p) = std::env::current_exe()
            && let Ok(mut f) = OpenOptions::new()
                .create(true)
                .append(true)
                .open(p.with_file_name("host-errors.log"))
        {
            let _ = writeln!(f, "{e}");
        }
        ServiceExitCode::ServiceSpecific(1)
    } else {
        ServiceExitCode::Win32(0)
    };
    // The host never restart_policies children itself. Only SCM recovery handles failures.
    let _ = report(&h, ServiceState::Stopped, code, 0, Duration::ZERO);
}
struct Handle(HANDLE);
impl Handle {
    fn new(h: HANDLE) -> io::Result<Self> {
        if h.is_null() {
            Err(io::Error::last_os_error())
        } else {
            Ok(Self(h))
        }
    }
}
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}
fn check(ok: i32) -> io::Result<()> {
    if ok == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}
fn supervise(
    s: &ServiceSpec,
    h: &ServiceStatusHandle,
    rx: mpsc::Receiver<()>,
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = std::env::current_exe()?
        .parent()
        .ok_or("missing host directory")?
        .to_path_buf();
    let out = OpenOptions::new()
        .create(true)
        .append(true)
        .open(directory.join("stdout.log"))?;
    let err = OpenOptions::new()
        .create(true)
        .append(true)
        .open(directory.join("stderr.log"))?;
    let input = OpenOptions::new().read(true).open("NUL")?;
    let event_name = format!("Local\\ServiceKit.{}.{}", s.id.as_str(), std::process::id());
    let event =
        Handle::new(unsafe { CreateEventW(ptr::null(), 1, 0, wide(&event_name).as_ptr()) })?;
    let mut environment = BTreeMap::<String, String>::new();
    for (k, v) in std::env::vars_os() {
        environment.insert(
            k.to_string_lossy().to_uppercase(),
            v.to_string_lossy().into(),
        );
    }
    for (k, v) in &s.environment {
        environment.insert(k.to_uppercase(), v.clone());
    }
    environment.insert("SERVICE_KIT_STOP_EVENT".into(), event_name);
    let mut env_block = vec![];
    for (k, v) in environment {
        env_block.extend(wide(&format!("{k}={v}")));
    }
    env_block.push(0);
    let job = Handle::new(unsafe { CreateJobObjectW(ptr::null(), ptr::null()) })?;
    let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { mem::zeroed() };
    limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    unsafe {
        check(SetInformationJobObject(
            job.0,
            JobObjectExtendedLimitInformation,
            &limits as *const _ as *const _,
            mem::size_of_val(&limits) as u32,
        ))?;
    }
    let mut startup: STARTUPINFOW = unsafe { mem::zeroed() };
    startup.cb = mem::size_of_val(&startup) as u32;
    startup.dwFlags = STARTF_USESTDHANDLES;
    startup.hStdOutput = out.as_raw_handle();
    startup.hStdError = err.as_raw_handle();
    startup.hStdInput = input.as_raw_handle();
    for handle in [startup.hStdOutput, startup.hStdError, startup.hStdInput] {
        unsafe {
            check(SetHandleInformation(
                handle,
                HANDLE_FLAG_INHERIT,
                HANDLE_FLAG_INHERIT,
            ))?;
        }
    }
    let mut command = wide(
        &std::iter::once(&s.executable)
            .chain(s.args.iter())
            .map(|a| quote_windows_arg(a))
            .collect::<Vec<_>>()
            .join(" "),
    );
    let cwd = s.working_directory.as_ref().map(|p| wide(p));
    let mut pi: PROCESS_INFORMATION = unsafe { mem::zeroed() };
    unsafe {
        check(CreateProcessW(
            wide(&s.executable).as_ptr(),
            command.as_mut_ptr(),
            ptr::null(),
            ptr::null(),
            1,
            CREATE_SUSPENDED | CREATE_UNICODE_ENVIRONMENT | CREATE_NO_WINDOW,
            env_block.as_ptr() as *const _,
            cwd.as_ref().map_or(ptr::null(), |v| v.as_ptr()),
            &startup,
            &mut pi,
        ))?;
    }
    let process = Handle::new(pi.hProcess)?;
    let thread = Handle::new(pi.hThread)?;
    // Assign while suspended: descendants can never escape before Job Object membership.
    unsafe {
        if AssignProcessToJobObject(job.0, process.0) == 0 {
            let e = io::Error::last_os_error();
            TerminateProcess(process.0, 1);
            return Err(e.into());
        }
        if ResumeThread(thread.0) == u32::MAX {
            let e = io::Error::last_os_error();
            TerminateJobObject(job.0, 1);
            return Err(e.into());
        }
    }
    report(
        h,
        ServiceState::Running,
        ServiceExitCode::Win32(0),
        0,
        Duration::ZERO,
    )?;
    let timeout = Duration::from_secs(u64::from(
        s.backend_options
            .scm
            .as_ref()
            .map_or(20, |o| o.stop_timeout_secs),
    ));
    let mut stopping = None;
    loop {
        if stopping.is_none() && rx.try_recv().is_ok() {
            stopping = Some(Instant::now());
            report(
                h,
                ServiceState::StopPending,
                ServiceExitCode::Win32(0),
                1,
                timeout,
            )?;
            // Cooperative children may open SERVICE_KIT_STOP_EVENT. Other children are
            // terminated as a process tree after the configured stop timeout.
            unsafe {
                check(SetEvent(event.0))?;
            }
        }
        if unsafe { WaitForSingleObject(process.0, 100) } == WAIT_OBJECT_0 {
            let mut code = 0;
            unsafe {
                check(GetExitCodeProcess(process.0, &mut code))?;
            }
            return if stopping.is_some() || code == 0 {
                Ok(())
            } else {
                Err(format!("child exited with code {code}").into())
            };
        }
        if let Some(start) = stopping {
            if start.elapsed() >= timeout {
                unsafe {
                    check(TerminateJobObject(job.0, 0))?;
                    WaitForSingleObject(process.0, 5000);
                }
                return Ok(());
            }
            report(
                h,
                ServiceState::StopPending,
                ServiceExitCode::Win32(0),
                2 + (start.elapsed().as_secs() as u32),
                timeout.saturating_sub(start.elapsed()),
            )?;
        }
    }
}
