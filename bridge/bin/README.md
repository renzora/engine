# Draco Encoder Binary Setup

This directory should contain the Google Draco encoder binary for 3D mesh compression.

## Building the Draco Encoder

### Prerequisites
- CMake 3.12 or higher
- C++ compiler (Visual Studio 2019+ on Windows, GCC/Clang on Linux/macOS)
- Git

### Windows Build Instructions

1. **Install Build Tools**
   ```cmd
   # Install Visual Studio Build Tools or Visual Studio Community
   # Make sure to include C++ build tools and CMake
   ```

2. **Clone and Build Draco**
   ```cmd
   git clone https://github.com/google/draco.git
   cd draco
   mkdir build
   cd build
   
   # Configure with CMake
   cmake .. -DCMAKE_BUILD_TYPE=Release -DDRACO_POINT_CLOUD_COMPRESSION=ON -DDRACO_MESH_COMPRESSION=ON
   
   # Build the project
   cmake --build . --config Release
   ```

3. **Copy the Binary**
   ```cmd
   # Copy the encoder to the bridge bin directory
   copy Release\draco_encoder.exe C:\path\to\engine\bridge\bin\
   ```

### Linux/macOS Build Instructions

1. **Install Dependencies**
   ```bash
   # Ubuntu/Debian
   sudo apt-get update
   sudo apt-get install cmake build-essential git
   
   # macOS (with Homebrew)
   brew install cmake
   ```

2. **Clone and Build Draco**
   ```bash
   git clone https://github.com/google/draco.git
   cd draco
   mkdir build
   cd build
   
   # Configure with CMake
   cmake .. -DCMAKE_BUILD_TYPE=Release -DDRACO_POINT_CLOUD_COMPRESSION=ON -DDRACO_MESH_COMPRESSION=ON
   
   # Build the project
   make -j$(nproc)
   ```

3. **Copy the Binary**
   ```bash
   # Copy the encoder to the bridge bin directory
   cp draco_encoder /path/to/engine/bridge/bin/
   ```

## Binary Requirements

The bridge expects to find:
- **Windows**: `draco_encoder.exe`
- **Linux/macOS**: `draco_encoder`

## Command Line Usage

The Draco encoder supports various options:
```bash
draco_encoder -i input.obj -o output.drc -cl 7 -qp 14
```

Where:
- `-i`: Input mesh file (OBJ, PLY, etc.)
- `-o`: Output compressed file (.drc)
- `-cl`: Compression level (0-10, higher = better compression)
- `-qp`: Position quantization bits (higher = better quality)

## Supported Formats

**Input formats:**
- OBJ
- PLY
- GLTF (experimental)

**Output format:**
- DRC (Draco compressed format)

## Integration

The Rust bridge will:
1. Convert mesh data to temporary OBJ format
2. Call the Draco encoder binary
3. Read the compressed output
4. Log compression statistics
5. Clean up temporary files

## Troubleshooting

**Binary not found:**
- Ensure the binary is in the `bridge/bin/` directory
- Check that the binary has execute permissions on Linux/macOS
- Verify the binary name matches the expected name for your platform

**Compression fails:**
- Check that input mesh data is valid
- Ensure sufficient disk space for temporary files
- Verify the Draco encoder version supports your input format

## Performance

Typical compression ratios:
- 3D models: 6x to 14x compression
- Point clouds: 10x to 20x compression
- Animation data: 8x to 16x compression

The compression level (7) and quantization bits (14) provide a good balance between file size and quality.