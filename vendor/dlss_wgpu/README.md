# dlss_wgpu - Deep Learning Super Sampling for wgpu

A wrapper for using [DLSS](https://www.nvidia.com/en-us/geforce/technologies/dlss) with [wgpu](https://github.com/gfx-rs/wgpu) when targeting Vulkan.

## Version Chart

| dlss_wgpu |   dlss   | wgpu  |
| :-------: | :------: | :---: |
|  v6.0.0   | v310.9.1 |  v30  |
|  v5.0.0   | v310.7.0 |  v30  |
|  v4.0.0   | v310.5.3 |  v29  |
|  v3.0.0   | v310.5.0 |  v28  |
|  v2.0.0   | v310.4.0 |  v27  |
|  v1.0.1   | v310.4.0 |  v26  |
|  v1.0.0   | v310.3.0 |  v26  |

## Downloading The DLSS SDK

The DLSS SDK cannot be redistributed by this crate. You will need to download the SDK as follows:

* Ensure you comply with the [DLSS SDK license](https://github.com/NVIDIA/DLSS/blob/v310.9.1/LICENSE.txt)
* Clone the [NVIDIA DLSS Super Resolution SDK v310.9.1](https://github.com/NVIDIA/DLSS/tree/v310.9.1)
* Set the environment variable `DLSS_SDK = /path/to/DLSS` (must be an absolute path)

## Build Dependencies

* Install the DLSS SDK
* Install the [Vulkan SDK](https://vulkan.lunarg.com/sdk/home) and set the `VULKAN_SDK` environment variable
* Install [clang](https://rust-lang.github.io/rust-bindgen/requirements.html#clang)

## Distributing Your App

Once your app is compiled, you do not need to distribute the entire DLSS SDK, or set the `DLSS_SDK` environment variable. You only need to distribute the DLSS DLL(s) and license text as follows:

1. Copy the DLL:
    * Windows: Copy `$DLSS_SDK/lib/Windows_x86_64/rel/nvngx_dlss.dll` to the same directory as your app
    * Linux: Copy `$DLSS_SDK/lib/Linux_x86_64/rel/libnvidia-ngx-dlss.so.310.9.1` to the same directory as your app
2. Include the full copyright and license blurb texts from section `9.5` of `$DLSS_SDK/doc/DLSS_Programming_Guide_Release.pdf` with your app
3. Additionally, for DLSS ray reconstruction:
    * Windows: Copy `$DLSS_SDK/lib/Windows_x86_64/rel/nvngx_dlssd.dll` to the same directory as your app
    * Linux: Copy `$DLSS_SDK/lib/Linux_x86_64/rel/libnvidia-ngx-dlssd.so.310.9.1` to the same directory as your app

## Debug Overlay

When `dlss_wgpu` is compiled with the `debug_overlay` cargo feature, and the `DLSS_SDK` environment variable is set, the development version of the DLSS DLLs will be linked.

The development version of the DLSS SDK comes with an in-app overlay to help debug usage of DLSS. See section `8.2` of `$DLSS_SDK/doc/DLSS_Programming_Guide_Release.pdf` for details.

## Validation Errors

Due to a bug in DLSS, you should [expect to see Vulkan validation errors](https://forums.developer.nvidia.com/t/validation-errors-using-dlss-vulkan-sdk-due-to-vkcmdclearcolorimage/326493).

These errors are safe to ignore.

## Contributing

You can run cargo commands without polluting your local environment by running:

```sh
docker run -v `pwd`:/src $(docker build -q .) cargo clippy
```

You must comply with the DLSS license to run this command.
