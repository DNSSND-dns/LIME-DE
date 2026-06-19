//! DRM device bring-up and KMS resource discovery.

use std::{io, path::Path};

use smithay::{
    backend::drm::{DrmDevice, DrmDeviceFd, DrmDeviceNotifier},
    reexports::drm::control::{connector, Device as ControlDevice, Mode, ModeTypeFlags},
    utils::DeviceFd,
};

use super::session::NativeSessionState;

#[derive(Debug)]
pub struct NativeDrmState {
    fd: DrmDeviceFd,
    device: Option<DrmDevice>,
    notifier: Option<DrmDeviceNotifier>,
    connected_outputs: usize,
}

impl NativeDrmState {
    pub fn new(
        session: &mut NativeSessionState,
        primary_gpu: &Path,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        println!("[native] DRM node: {}", primary_gpu.display());

        let fd = session.open_drm_node(primary_gpu)?;
        let fd = DrmDeviceFd::new(DeviceFd::from(fd));
        let (device, notifier) = DrmDevice::new(fd.clone(), false).map_err(|error| {
            io::Error::other(format!(
                "[native] failed to initialize DRM device {}: {error}",
                primary_gpu.display()
            ))
        })?;

        let connected_outputs = log_resources(&device)?;
        if connected_outputs == 0 {
            return Err(io::Error::other(
                "[native] DRM device opened, but no connected connectors found",
            )
            .into());
        }

        Ok(Self {
            fd,
            device: Some(device),
            notifier: Some(notifier),
            connected_outputs,
        })
    }

    #[must_use]
    pub fn connected_outputs(&self) -> usize {
        self.connected_outputs
    }

    pub(crate) fn take_notifier(
        &mut self,
    ) -> Result<DrmDeviceNotifier, Box<dyn std::error::Error>> {
        self.notifier
            .take()
            .ok_or_else(|| io::Error::other("DRM notifier was already registered").into())
    }

    #[must_use]
    pub(crate) fn fd(&self) -> DrmDeviceFd {
        self.fd.clone()
    }

    pub(crate) fn take_device(&mut self) -> Result<DrmDevice, Box<dyn std::error::Error>> {
        self.device
            .take()
            .ok_or_else(|| io::Error::other("DRM device was already assigned to an output").into())
    }
}

fn log_resources(device: &DrmDevice) -> Result<usize, Box<dyn std::error::Error>> {
    let resources = device.resource_handles().map_err(|error| {
        io::Error::other(format!("[native] failed to read DRM resources: {error}"))
    })?;

    println!("[native] crtcs: {:?}", resources.crtcs());
    for crtc in resources.crtcs() {
        match device.get_crtc(*crtc) {
            Ok(info) => println!(
                "[native] crtc: {crtc:?}, position: {:?}, mode: {}, framebuffer: {:?}",
                info.position(),
                info.mode()
                    .as_ref()
                    .map(format_mode)
                    .unwrap_or_else(|| "none".to_string()),
                info.framebuffer()
            ),
            Err(error) => eprintln!("[native] failed to read CRTC {crtc:?}: {error}"),
        }
    }
    println!("[native] encoders: {:?}", resources.encoders());

    match device.plane_handles() {
        Ok(planes) => {
            println!("[native] planes: {planes:?}");
            for plane in planes {
                match device.get_plane(plane) {
                    Ok(info) => println!(
                        "[native] plane: {plane:?}, possible crtcs: {:?}, formats: {:?}",
                        resources.filter_crtcs(info.possible_crtcs()),
                        info.formats()
                    ),
                    Err(error) => {
                        eprintln!("[native] failed to read plane {plane:?}: {error}");
                    }
                }
            }
        }
        Err(error) => eprintln!("[native] planes unavailable: {error}"),
    }

    let mut connected_outputs = 0;
    for handle in resources.connectors() {
        let info = device.get_connector(*handle, true).map_err(|error| {
            io::Error::other(format!(
                "[native] failed to read connector {handle:?}: {error}"
            ))
        })?;
        let name = format!("{}-{}", info.interface().as_str(), info.interface_id());
        let status = connector_status(info.state());
        println!("[native] connector: {name} {status}");

        if info.state() != connector::State::Connected {
            continue;
        }

        connected_outputs += 1;
        if info.modes().is_empty() {
            println!("[native] available modes: none");
        } else {
            let modes = info
                .modes()
                .iter()
                .map(format_mode)
                .collect::<Vec<_>>()
                .join(", ");
            println!("[native] available modes: {modes}");
        }

        if let Some(preferred) = info
            .modes()
            .iter()
            .find(|mode| mode.mode_type().contains(ModeTypeFlags::PREFERRED))
            .or_else(|| info.modes().first())
        {
            println!("[native] preferred mode: {}", format_mode(preferred));
        } else {
            println!("[native] preferred mode: none");
        }

        let connector_crtcs = info
            .encoders()
            .iter()
            .filter_map(|encoder| device.get_encoder(*encoder).ok())
            .flat_map(|encoder| resources.filter_crtcs(encoder.possible_crtcs()))
            .collect::<Vec<_>>();
        println!("[native] connector crtcs: {connector_crtcs:?}");
    }

    Ok(connected_outputs)
}

fn connector_status(state: connector::State) -> &'static str {
    match state {
        connector::State::Connected => "connected",
        connector::State::Disconnected => "disconnected",
        connector::State::Unknown => "unknown",
    }
}

fn format_mode(mode: &Mode) -> String {
    let (width, height) = mode.size();
    format!("{width}x{height}@{}", mode.vrefresh())
}
