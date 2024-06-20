use ash::Entry;

fn main() {
    let entry = Entry::linked();

    let res = entry.try_enumerate_instance_version();
    if res.is_err() {
        println!("cargo::rustc-cfg=vulkan_1_0");
    }
    let res = res.unwrap();
    if res.is_none() {
        panic!("No Vulkan Version found. Check if the Vulkan SDK is properly installed.");
    }

    let version = res.unwrap();
    let implemented_vulkan_versions = [
        //(ash::vk::make_api_version(0, 1, 3, 0), "vulkan_1_3"),
        (ash::vk::make_api_version(0, 1, 2, 0), "vulkan_1_2"),
        (ash::vk::make_api_version(0, 1, 1, 0), "vulkan_1_1"),
        (ash::vk::make_api_version(0, 1, 0, 0), "vulkan_1_0"),
    ];

    for (test_version, name) in implemented_vulkan_versions {
        if version >= test_version {
            println!("cargo::rustc-cfg={}", name);
            break
        }
    }
}