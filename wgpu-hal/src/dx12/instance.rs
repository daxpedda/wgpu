use winapi::shared::{dxgi1_5, minwindef};

use super::SurfaceTarget;
use crate::auxil::{self, dxgi::result::HResult as _};
use std::{mem, sync::Arc};

impl Drop for super::Instance {
    fn drop(&mut self) {
        crate::auxil::dxgi::exception::unregister_exception_handler();
    }
}

impl crate::Instance<super::Api> for super::Instance {
    unsafe fn init(desc: &crate::InstanceDescriptor) -> Result<Self, crate::InstanceError> {
        let lib_main = d3d12::D3D12Lib::new().map_err(|e| {
            crate::InstanceError::with_source(String::from("failed to load d3d12.dll"), e)
        })?;

        if desc.flags.contains(wgt::InstanceFlags::VALIDATION) {
            // Enable debug layer
            match lib_main.get_debug_interface() {
                Ok(pair) => match pair.into_result() {
                    Ok(debug_controller) => {
                        debug_controller.enable_layer();
                    }
                    Err(err) => {
                        log::warn!("Unable to enable D3D12 debug interface: {}", err);
                    }
                },
                Err(err) => {
                    log::warn!("Debug interface function for D3D12 not found: {:?}", err);
                }
            }
        }

        // Create DXGIFactory4
        let (lib_dxgi, factory) = auxil::dxgi::factory::create_factory(
            auxil::dxgi::factory::DxgiFactoryType::Factory4,
            desc.flags,
        )?;

        // Create IDXGIFactoryMedia
        let factory_media = match lib_dxgi.create_factory_media() {
            Ok(pair) => match pair.into_result() {
                Ok(factory_media) => Some(factory_media),
                Err(err) => {
                    log::error!("Failed to create IDXGIFactoryMedia: {}", err);
                    None
                }
            },
            Err(err) => {
                log::info!("IDXGIFactory1 creation function not found: {:?}", err);
                None
            }
        };

        let mut supports_allow_tearing = false;
        #[allow(trivial_casts)]
        if let Some(factory5) = factory.as_factory5() {
            let mut allow_tearing: minwindef::BOOL = minwindef::FALSE;
            let hr = unsafe {
                factory5.CheckFeatureSupport(
                    dxgi1_5::DXGI_FEATURE_PRESENT_ALLOW_TEARING,
                    &mut allow_tearing as *mut _ as *mut _,
                    mem::size_of::<minwindef::BOOL>() as _,
                )
            };

            match hr.into_result() {
                Err(err) => log::warn!("Unable to check for tearing support: {}", err),
                Ok(()) => supports_allow_tearing = true,
            }
        }

        Ok(Self {
            // The call to create_factory will only succeed if we get a factory4, so this is safe.
            factory,
            factory_media,
            library: Arc::new(lib_main),
            _lib_dxgi: lib_dxgi,
            supports_allow_tearing,
            flags: desc.flags,
            dx12_shader_compiler: desc.dx12_shader_compiler.clone(),
        })
    }

    unsafe fn create_surface(
        &self,
        _display_handle: raw_window_handle_0_5::RawDisplayHandle,
        window_handle: raw_window_handle_0_5::RawWindowHandle,
    ) -> Result<super::Surface, crate::InstanceError> {
        match window_handle {
            raw_window_handle_0_5::RawWindowHandle::Win32(handle) => Ok(super::Surface {
                factory: self.factory.clone(),
                factory_media: self.factory_media.clone(),
                target: SurfaceTarget::WndHandle(handle.hwnd as *mut _),
                supports_allow_tearing: self.supports_allow_tearing,
                swap_chain: None,
            }),
            _ => Err(crate::InstanceError::new(format!(
                "window handle {window_handle:?} is not a Win32 handle"
            ))),
        }
    }
    #[cfg(feature = "raw-window-handle-0-6")]
    unsafe fn create_surface_0_6(
        &self,
        _display_handle: raw_window_handle_0_6::RawDisplayHandle,
        window_handle: raw_window_handle_0_6::RawWindowHandle,
    ) -> Result<super::Surface, crate::InstanceError> {
        match window_handle {
            raw_window_handle_0_6::RawWindowHandle::Win32(handle) => Ok(super::Surface {
                factory: self.factory.clone(),
                factory_media: self.factory_media.clone(),
                target: SurfaceTarget::WndHandle(handle.hwnd.get() as *mut _),
                supports_allow_tearing: self.supports_allow_tearing,
                swap_chain: None,
            }),
            _ => Err(crate::InstanceError::new(format!(
                "window handle {window_handle:?} is not a Win32 handle"
            ))),
        }
    }
    unsafe fn destroy_surface(&self, _surface: super::Surface) {
        // just drop
    }

    unsafe fn enumerate_adapters(&self) -> Vec<crate::ExposedAdapter<super::Api>> {
        let adapters = auxil::dxgi::factory::enumerate_adapters(self.factory.clone());

        adapters
            .into_iter()
            .filter_map(|raw| {
                super::Adapter::expose(raw, &self.library, self.flags, &self.dx12_shader_compiler)
            })
            .collect()
    }
}
