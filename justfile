set dotenv-load := true
set windows-shell := ["pwsh.exe", "-NoLogo", "-ExecutionPolicy", "RemoteSigned", "-Command"]

import "justfiles/setup.just"
import "justfiles/quality.just"
import "justfiles/test.just"
import "justfiles/build.just"
import "justfiles/maintenance.just"

default:
    @just --list
