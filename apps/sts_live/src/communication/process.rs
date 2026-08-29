use crate::model::{LiveError, LiveResult};
use std::process::Command;

pub(crate) fn process_exists(pid: u32) -> bool {
    #[cfg(windows)]
    {
        let command = format!(
            "if (Get-Process -Id {pid} -ErrorAction SilentlyContinue) {{ exit 0 }} else {{ exit 1 }}"
        );
        let output = Command::new("powershell.exe")
            .args(["-NoProfile", "-Command", &command])
            .output();
        output
            .map(|output| output.status.success())
            .unwrap_or(false)
    }
    #[cfg(not(windows))]
    {
        Command::new("kill")
            .args(["-0", &pid.to_string()])
            .status()
            .map(|status| status.success())
            .unwrap_or(true)
    }
}

pub(crate) fn process_is_nodejs(pid: u32) -> bool {
    #[cfg(windows)]
    {
        let command = format!(
            "$p = Get-Process -Id {pid} -ErrorAction SilentlyContinue; if ($p -and $p.Path -like '*\\nodejs\\node.exe') {{ exit 0 }} else {{ exit 1 }}"
        );
        let output = Command::new("powershell.exe")
            .args(["-NoProfile", "-Command", &command])
            .output();
        output
            .map(|output| output.status.success())
            .unwrap_or(false)
    }
    #[cfg(not(windows))]
    {
        Command::new("ps")
            .args(["-p", &pid.to_string(), "-o", "comm="])
            .output()
            .map(|output| {
                output.status.success()
                    && String::from_utf8_lossy(&output.stdout)
                        .trim()
                        .ends_with("node")
            })
            .unwrap_or(false)
    }
}

pub(crate) fn kill_process(pid: u32) -> LiveResult<()> {
    #[cfg(windows)]
    let mut command = {
        let mut command = Command::new("taskkill");
        command.args(["/PID", &pid.to_string(), "/F"]);
        command
    };
    #[cfg(not(windows))]
    let mut command = {
        let mut command = Command::new("kill");
        command.args(["-TERM", &pid.to_string()]);
        command
    };
    let output = command.output()?;
    if output.status.success() {
        Ok(())
    } else {
        Err(LiveError::Bridge(
            String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        ))
    }
}
