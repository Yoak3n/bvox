plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.compose)
}

// Location of the Cargo workspace (this module lives in <repo>/android/app).
val repoRoot = rootDir.parentFile
val rustTargets = listOf("x86_64", "arm64-v8a")

android {
    namespace = "dev.yoa.bilimusic"
    compileSdk = 37

    defaultConfig {
        applicationId = "dev.yoa.bilimusic"
        minSdk = 23
        targetSdk = 36
        versionCode = 1
        versionName = "0.1.0"
        ndkVersion = "29.0.14206865"
    }

    buildTypes {
        release {
            isMinifyEnabled = false
        }
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    buildFeatures {
        compose = true
    }
}

// Cross-compile bili-core for the Android ABIs. cargo-ndk resolves the NDK
// linkers from ANDROID_HOME/ANDROID_NDK_HOME and copies each .so into
// <abi>/libbili_core.so under rustJniLibs.
tasks.register<Exec>("buildRust") {
    workingDir(repoRoot)
    commandLine(
        "cargo", "ndk",
        "--platform", "23",
        *rustTargets.flatMap { listOf("-t", it) }.toTypedArray(),
        "-o", layout.buildDirectory.file("rustJniLibs").get().asFile.absolutePath,
        "build", "--release", "-p", "bili-core",
    )
    inputs.dir(repoRoot.resolve("crates/core/src"))
    inputs.file(repoRoot.resolve("crates/core/Cargo.toml"))
    outputs.dir(layout.buildDirectory.file("rustJniLibs").get().asFile.absolutePath)
}

// Generate the UniFFI Kotlin bindings from the x86_64 cdylib (library mode
// reads the embedded metadata; no execution involved).
tasks.register<Exec>("generateUniffi") {
    dependsOn("buildRust")
    workingDir(repoRoot)
    val library = layout.buildDirectory.file("rustJniLibs/x86_64/libbili_core.so").get().asFile
    commandLine(
        "cargo", "run", "--features", "uniffi-cli", "-p", "bili-core",
        "--bin", "uniffi-bindgen",
        "generate",
        "--library", library.absolutePath,
        "--language", "kotlin",
        "--out-dir", layout.buildDirectory.file("generated/uniffi/kotlin").get().asFile.absolutePath,
        "--config", "crates/core/uniffi.toml",
    )
    inputs.file(library.absolutePath)
    outputs.dir(layout.buildDirectory.file("generated/uniffi/kotlin").get().asFile.absolutePath)
}

// AGP 9 requires source dirs through the Variant API, not sourceSets.
// Ordering is ensured by the preBuild dependency below.
androidComponents {
    onVariants { variant ->
        variant.sources.java?.addStaticSourceDirectory(
            layout.buildDirectory.dir("generated/uniffi/kotlin").get().asFile.absolutePath
        )
        variant.sources.jniLibs?.addStaticSourceDirectory(
            layout.buildDirectory.dir("rustJniLibs").get().asFile.absolutePath
        )
    }
}

tasks.named("preBuild") {
    dependsOn("generateUniffi")
}

dependencies {
    implementation(platform(libs.androidx.compose.bom))
    implementation(libs.androidx.activity.compose)
    implementation(libs.androidx.core.ktx)
    implementation(libs.androidx.compose.ui)
    implementation(libs.androidx.compose.ui.graphics)
    implementation(libs.androidx.compose.material3)
    implementation(libs.androidx.media3.exoplayer)
    implementation(libs.androidx.media3.session)
    implementation(libs.jna) {
        artifact {
            type = "aar"
        }
    }
}
