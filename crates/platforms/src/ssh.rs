use async_trait::async_trait;
use shiftwrangler_core::{
    error::{Result, ShiftError},
    platform::{Platform, PlatformMode, Target},
};
use tracing::info;

/// Manages a remote machine via SSH commands and Wake-on-LAN.
pub struct SshPlatform;

impl SshPlatform {
    pub fn new() -> Self {
        Self
    }
}

impl Default for SshPlatform {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Platform for SshPlatform {
    fn mode(&self) -> PlatformMode {
        PlatformMode::Ssh
    }

    async fn suspend(&self, target: &Target) -> Result<()> {
        let host = target
            .host
            .as_deref()
            .ok_or_else(|| ShiftError::Platform("SSH target has no host".into()))?;

        info!(%host, "suspending remote machine via SSH");

        run_ssh_command(target, "sudo systemctl suspend").await
    }

    async fn wake(&self, target: &Target) -> Result<()> {
        let mac = target
            .mac_address
            .as_deref()
            .ok_or_else(|| ShiftError::Platform("SSH target has no MAC address for WoL".into()))?;

        info!(%mac, "sending Wake-on-LAN magic packet");
        send_wol(mac).await
    }

    async fn is_alive(&self, target: &Target) -> Result<bool> {
        let host = target
            .host
            .as_deref()
            .ok_or_else(|| ShiftError::Platform("SSH target has no host".into()))?;

        let result = run_ssh_command(target, "true").await;
        let _ = host;
        Ok(result.is_ok())
    }
}

async fn run_ssh_command(target: &Target, command: &str) -> Result<()> {
    let host = target.host.as_deref().unwrap_or("");
    let port = target.ssh_port.unwrap_or(22);

    let mut args = vec![
        "-o", "StrictHostKeyChecking=no",
        "-o", "ConnectTimeout=10",
        "-p", Box::leak(port.to_string().into_boxed_str()),
    ];

    let key_arg;
    if let Some(key) = &target.ssh_key {
        key_arg = key.display().to_string();
        args.push("-i");
        args.push(Box::leak(key_arg.into_boxed_str()));
    }

    args.push(host);
    args.push(command);

    let status = tokio::process::Command::new("ssh")
        .args(&args)
        .status()
        .await
        .map_err(|e| ShiftError::Platform(format!("ssh exec failed: {e}")))?;

    if !status.success() {
        return Err(ShiftError::Platform(format!(
            "ssh command '{command}' returned non-zero on {host}"
        )));
    }
    Ok(())
}

/// Send a Wake-on-LAN magic packet to the given MAC address via UDP broadcast.
async fn send_wol(mac: &str) -> Result<()> {
    use std::net::UdpSocket;

    let mac_bytes = parse_mac(mac)?;
    let mut packet = vec![0xff_u8; 6];
    for _ in 0..16 {
        packet.extend_from_slice(&mac_bytes);
    }

    let socket = UdpSocket::bind("0.0.0.0:0")
        .map_err(|e| ShiftError::Platform(format!("WoL socket bind failed: {e}")))?;
    socket
        .set_broadcast(true)
        .map_err(|e| ShiftError::Platform(format!("WoL set_broadcast failed: {e}")))?;
    socket
        .send_to(&packet, "255.255.255.255:9")
        .map_err(|e| ShiftError::Platform(format!("WoL send failed: {e}")))?;

    Ok(())
}

fn parse_mac(mac: &str) -> Result<[u8; 6]> {
    let parts: Vec<&str> = mac.split([':', '-']).collect();
    if parts.len() != 6 {
        return Err(ShiftError::Platform(format!("invalid MAC address: {mac}")));
    }
    let mut bytes = [0u8; 6];
    for (i, part) in parts.iter().enumerate() {
        bytes[i] = u8::from_str_radix(part, 16)
            .map_err(|_| ShiftError::Platform(format!("invalid MAC octet: {part}")))?;
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_mac_colon_separated() {
        let bytes = parse_mac("aa:bb:cc:dd:ee:ff").unwrap();
        assert_eq!(bytes, [0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff]);
    }

    #[test]
    fn parse_mac_dash_separated() {
        let bytes = parse_mac("aa-bb-cc-dd-ee-ff").unwrap();
        assert_eq!(bytes, [0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff]);
    }

    #[test]
    fn parse_mac_rejects_invalid() {
        assert!(parse_mac("not-a-mac").is_err());
        assert!(parse_mac("aa:bb:cc").is_err());
    }

    #[tokio::test]
    async fn is_alive_fails_without_host() {
        let p = SshPlatform::new();
        let target = Target::local(); // no host set
        let result = p.is_alive(&target).await;
        assert!(result.is_err());
    }
}
