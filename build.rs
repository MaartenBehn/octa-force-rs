fn main() {

    /*
    // Find supported Vulkan Version
        let implemented_vulkan_versions = [
            VK_1_0,
            VK_1_1,
            VK_1_2,
            VK_1_3
        ];

        let res = entry.try_enumerate_instance_version();
        if res.is_err() {
            bail!("No Vulkan Version found. Check if the Vulkan SDK is properly installed.");
        }
        let res = res.unwrap();
        if res.is_none() {
            bail!("No Vulkan Version found. Check if the Vulkan SDK is properly installed.");
        }

        let version = res.unwrap();
        let mut supported_vulkan_versions = vec![];
        for test_version in implemented_vulkan_versions {
            if version >= test_version.make_api_version() {
                supported_vulkan_versions.push(test_version)
            }
        }

        if supported_vulkan_versions.is_empty() {
            bail!("Vulkan Version is not supported by octaforce. The lowest supported Vulkan Version is 1.2. ");
        }

        let picked_version = if engine_config.wanted_vulkan_version.is_some() {
            let mut found = true;
            for test_version in supported_vulkan_versions.iter() {
                if test_version.make_api_version() == engine_config.wanted_vulkan_version.unwrap().make_api_version() {
                    found = true;
                }
            }

            if found {
                engine_config.wanted_vulkan_version.unwrap()
            } else {
                info!("Wanted Vulkan Version {:?} not supported", engine_config.wanted_vulkan_version.unwrap());
                supported_vulkan_versions[supported_vulkan_versions.len() - 1]
            }
        } else {
            supported_vulkan_versions[supported_vulkan_versions.len() - 1]
        };
        info!("Using Vulkan Version {:?}", picked_version);
     */

    println!("cargo::rustc-cfg=vulkan_1_2");
}