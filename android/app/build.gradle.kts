plugins {
    id("com.android.application")
}

android {
    namespace = "com.renzora.runtime"
    compileSdk = 34

    defaultConfig {
        applicationId = "com.renzora.runtime"
        minSdk = 30
        targetSdk = 34
        versionCode = 1
        versionName = "0.2.0"

        ndk {
            abiFilters += listOf("arm64-v8a", "x86_64")
        }
    }

    buildFeatures {
        prefab = true
    }

    flavorDimensions += "device"
    productFlavors {
        create("standard") {
            dimension = "device"
        }
        create("firetv") {
            dimension = "device"
            applicationIdSuffix = ".firetv"
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = false
        }
    }

    sourceSets {
        getByName("main") {
            // Native library built by cargo ndk goes here
            jniLibs.srcDirs("src/main/jniLibs")
        }
    }
}

configurations.all {
    resolutionStrategy {
        force("org.jetbrains.kotlin:kotlin-stdlib:1.8.22")
        force("org.jetbrains.kotlin:kotlin-stdlib-jdk8:1.8.22")
    }
}

dependencies {
    implementation("androidx.games:games-activity:2.0.2")
    implementation("androidx.appcompat:appcompat:1.7.0")
}
