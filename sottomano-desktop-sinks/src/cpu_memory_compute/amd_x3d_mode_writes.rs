use std::fs;

use sottomano_desktop_sources::{
    cpu_memory_compute::amd_x3d_cache_preference::{X3D_VCACHE_DRIVER, X3dCachePreference},
    system_protocol_clients::sysfs_attribute_reads::Error,
};

pub fn write_x3d_cache_preference(preference: X3dCachePreference) -> Result<(), Error> {
    let mode_file = X3dCachePreference::mode_file()
        .ok_or_else(|| Error::Unreadable(X3D_VCACHE_DRIVER.to_owned()))?;
    fs::write(&mode_file, preference.stored_name()).map_err(|failure| Error::Unwritable {
        path: mode_file.display().to_string(),
        detail: failure.to_string(),
    })
}
