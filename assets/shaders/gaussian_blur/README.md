# Gaussian Blur

A plain Gaussian blur over the whole frame. `sigma` sets how soft it is, `kernel_size` how many samples pay for it, so raise them together: a large sigma through a small kernel is a blur with banding.

**Use it:** add a **Post Process** component to the camera or the World Environment, then drag `gaussian_blur.wesl` from the Assets panel onto it.

**Settings:** Sigma, Kernel Size
