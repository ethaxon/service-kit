//! SCM API adapter; no sc.exe parsing, wrappers, or scheduled-task fallback.
use super::*;
use std::{
    ffi::OsString,
    fs,
    time::{Duration, Instant},
};
use windows_service::{
    service::{
        Service, ServiceAccess, ServiceAction, ServiceActionType, ServiceErrorControl,
        ServiceFailureActions, ServiceFailureResetPeriod, ServiceInfo, ServiceStartType,
        ServiceType,
    },
    service_manager::{ServiceManager, ServiceManagerAccess},
};
fn err(e: windows_service::Error) -> Error {
    Error::new(ErrorCode::ManagerFailed, e.to_string())
}
fn manager(access: ServiceManagerAccess) -> Result<ServiceManager> {
    ServiceManager::local_computer(None::<&str>, access).map_err(err)
}
pub fn probe(mut p: BackendCandidate) -> BackendCandidate {
    p.manager_available = true;
    p.reachable = manager(ServiceManagerAccess::CONNECT).is_ok();
    p.can_write =
        manager(ServiceManagerAccess::CONNECT | ServiceManagerAccess::CREATE_SERVICE).is_ok();
    if !p.reachable {
        p.diagnostics.push("SCM connection failed".into());
    }
    if !p.can_write {
        p.diagnostics
            .push("SCM create-service access denied; run elevated".into());
    }
    p.diagnostics
        .push("experimental: compiled host requires real Windows lifecycle verification".into());
    p
}
fn name(s: &ServiceSpec) -> String {
    format!("service-kit-{}", s.id.as_str())
}
fn protect(path: &std::path::Path, descriptor_text: &str) -> Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::{
        Foundation::LocalFree,
        Security::{Authorization::*, *},
    };
    files::reject_links(path)?;
    let sddl: Vec<u16> = descriptor_text.encode_utf16().chain(Some(0)).collect();
    let name: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let mut descriptor = std::ptr::null_mut();
    unsafe {
        if ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            1,
            &mut descriptor,
            std::ptr::null_mut(),
        ) == 0
        {
            return Err(std::io::Error::last_os_error().into());
        }
        let mut present = 0;
        let mut defaulted = 0;
        let mut dacl = std::ptr::null_mut();
        if GetSecurityDescriptorDacl(descriptor, &mut present, &mut dacl, &mut defaulted) == 0
            || present == 0
        {
            LocalFree(descriptor);
            return Err(Error::new(
                ErrorCode::PermissionDenied,
                "cannot construct protected host ACL",
            ));
        }
        let error = SetNamedSecurityInfoW(
            name.as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            dacl,
            std::ptr::null(),
        );
        LocalFree(descriptor);
        if error != 0 {
            return Err(std::io::Error::from_raw_os_error(error as i32).into());
        }
    }
    Ok(())
}
fn status(service: &Service) -> Result<ServiceStatus> {
    let s = service.query_status().map_err(err)?;
    Ok(ServiceStatus {
        state: match s.current_state {
            windows_service::service::ServiceState::Running => {
                service_kit_core::ServiceState::Running
            }
            windows_service::service::ServiceState::Stopped => {
                service_kit_core::ServiceState::Stopped
            }
            _ => service_kit_core::ServiceState::Unknown,
        },
        detail: format!("{:?}; exit={:?}", s.current_state, s.exit_code),
    })
}
fn wait(
    service: &Service,
    state: windows_service::service::ServiceState,
    timeout: Duration,
) -> Result<()> {
    let end = Instant::now() + timeout;
    loop {
        if service.query_status().map_err(err)?.current_state == state {
            return Ok(());
        }
        if Instant::now() >= end {
            return Err(Error::new(ErrorCode::Timeout, "SCM transition timed out"));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}
pub fn apply(s: &ServiceSpec, op: Operation, actions: &[PlanAction]) -> Result<ServiceStatus> {
    let m = manager(
        ServiceManagerAccess::CONNECT
            | if op == Operation::Install {
                ServiceManagerAccess::CREATE_SERVICE
            } else {
                ServiceManagerAccess::empty()
            },
    )?;
    let access = ServiceAccess::QUERY_CONFIG
        | ServiceAccess::QUERY_STATUS
        | match op {
            Operation::Status => ServiceAccess::empty(),
            Operation::Start => ServiceAccess::START,
            Operation::Stop => ServiceAccess::STOP,
            Operation::Restart => ServiceAccess::START | ServiceAccess::STOP,
            Operation::Uninstall => ServiceAccess::STOP | ServiceAccess::DELETE,
            Operation::Install => ServiceAccess::CHANGE_CONFIG,
        };
    let mut service = match m.open_service(name(s), access) {
        Ok(service) => Some(service),
        Err(windows_service::Error::Winapi(e)) if e.raw_os_error() == Some(1060) => None,
        Err(e) => return Err(err(e)),
    };
    let existing = service.is_some();
    let content = serde_json::to_string_pretty(s)
        .map_err(|e| Error::new(ErrorCode::InvalidSpec, e.to_string()))?;
    for action in actions {
        let PlanAction::Scm {
            step,
            only_if_absent,
        } = action
        else {
            return Err(Error::new(ErrorCode::InvalidSpec, "expected SCM step"));
        };
        if *only_if_absent && existing {
            continue;
        }
        match step {
            ScmStep::VerifyOwnership {
                directory,
                absent_is_success,
                ..
            } => {
                if op != Operation::Status {
                    files::reject_links(directory)?;
                }
                let spec_path = directory.join("spec.json");
                if let Some(service) = &service {
                    let config = service.query_config().map_err(err)?;
                    let expected = format!(
                        "{} {}",
                        directory.join("service-kit-host.exe").display(),
                        spec_path.display()
                    );
                    if config.executable_path.as_os_str() != std::ffi::OsStr::new(&expected) {
                        return Err(Error::new(
                            ErrorCode::Conflict,
                            "SCM name belongs to an unrelated service",
                        ));
                    }
                    if op != Operation::Status {
                        verify_file(&spec_path, &content)?;
                    }
                } else if op != Operation::Install {
                    if *absent_is_success {
                        return Ok(ServiceStatus {
                            state: ServiceState::NotInstalled,
                            detail: "SCM entry absent".into(),
                        });
                    }
                    return Err(Error::new(ErrorCode::NotFound, "SCM entry absent"));
                } else if directory.exists() && !spec_path.exists() {
                    return Err(Error::new(
                        ErrorCode::Conflict,
                        "unmanaged host directory exists",
                    ));
                }
            }
            ScmStep::EnsureProtectedDirectory { path, sddl } => {
                fs::create_dir_all(path)?;
                protect(path, sddl)?;
            }
            ScmStep::WriteSpec { path, content } => {
                ensure_file(path, content, false)?;
            }
            ScmStep::CopyHost {
                source,
                destination,
            } => {
                let bytes = fs::read(source)?;
                if destination.exists() {
                    if fs::read(destination)? != bytes {
                        return Err(Error::new(
                            ErrorCode::Conflict,
                            "host binary differs; uninstall before upgrading",
                        ));
                    }
                } else {
                    use std::io::Write;
                    let mut file = tempfile::NamedTempFile::new_in(destination.parent().unwrap())?;
                    file.write_all(&bytes)?;
                    file.as_file().sync_all()?;
                    file.persist_noclobber(destination)
                        .map_err(|e| Error::new(ErrorCode::Conflict, e.to_string()))?;
                }
            }
            ScmStep::CreateService {
                service_name,
                host,
                spec_path,
                startup,
                description,
            } => {
                let created = m
                    .create_service(
                        &ServiceInfo {
                            name: service_name.into(),
                            display_name: s.id.as_str().into(),
                            service_type: ServiceType::OWN_PROCESS,
                            start_type: if *startup == Startup::Automatic {
                                ServiceStartType::AutoStart
                            } else {
                                ServiceStartType::OnDemand
                            },
                            error_control: ServiceErrorControl::Normal,
                            executable_path: host.clone(),
                            launch_arguments: vec![spec_path.as_os_str().into()],
                            dependencies: vec![],
                            account_name: None,
                            account_password: None,
                        },
                        access,
                    )
                    .map_err(err)?;
                created.set_description(description).map_err(err)?;
                service = Some(created);
            }
            ScmStep::ConfigureRecovery {
                restart,
                reset_secs,
                delay_secs,
            } => {
                let service = service.as_ref().unwrap();
                service
                    .update_failure_actions(ServiceFailureActions {
                        reset_period: ServiceFailureResetPeriod::After(Duration::from_secs(
                            *reset_secs,
                        )),
                        reboot_msg: None,
                        command: None,
                        actions: Some(if *restart == RestartPolicy::OnFailure {
                            vec![ServiceAction {
                                action_type: ServiceActionType::Restart,
                                delay: Duration::from_secs(*delay_secs),
                            }]
                        } else {
                            vec![]
                        }),
                    })
                    .map_err(err)?;
                service
                    .set_failure_actions_on_non_crash_failures(true)
                    .map_err(err)?;
            }
            ScmStep::StopIfRunning { timeout_secs } => {
                let service = service.as_ref().unwrap();
                if service.query_status().map_err(err)?.current_state
                    != windows_service::service::ServiceState::Stopped
                {
                    service.stop().map_err(err)?;
                    wait(
                        service,
                        windows_service::service::ServiceState::Stopped,
                        Duration::from_secs(*timeout_secs),
                    )?;
                }
            }
            ScmStep::StartIfStopped { timeout_secs } => {
                let service = service.as_ref().unwrap();
                if service.query_status().map_err(err)?.current_state
                    != windows_service::service::ServiceState::Running
                {
                    service.start::<OsString>(&[]).map_err(err)?;
                    wait(
                        service,
                        windows_service::service::ServiceState::Running,
                        Duration::from_secs(*timeout_secs),
                    )?;
                }
            }
            ScmStep::DeleteService => {
                service.take().unwrap().delete().map_err(err)?;
            }
            ScmStep::RemoveHostFiles { directory, files } => {
                for file in files {
                    let path = directory.join(file);
                    match fs::remove_file(path) {
                        Ok(()) => {}
                        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                        Err(e) => return Err(e.into()),
                    }
                }
                fs::remove_dir(directory)?;
            }
            ScmStep::QueryStatus => return status(service.as_ref().unwrap()),
        }
    }
    Ok(ServiceStatus {
        state: ServiceState::NotInstalled,
        detail: "SCM entry and host files removed".into(),
    })
}
