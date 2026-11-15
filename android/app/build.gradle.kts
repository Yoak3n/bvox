import java.util.Properties

plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.compose)
}

// Location of the Cargo workspace (this module lives in <repo>/android/app).
val repoRoot = rootDir.parentFile
val rustTargets = listOf("x86_64", "arm64-v8a")

// Release signing material lives outside version control (see .gitignore).
// Without the file, `assembleRelease` falls back to an unsigned APK.
val keystorePropertiesFile = rootProject.file("keystore.properties")
val keystoreProperties = Properties().apply {
    if (keystorePropertiesFile.exists()) {
        keystorePropertiesFile.inputStream().use { load(it) }
    }
}

android {
    namespace = "dev.yoa.bvox"
    compileSdk = 37

    defaultConfig {
        applicationId = "dev.yoa.bvox"
        minSdk = 23
        targetSdk = 36
        versionCode = 5
        versionName = "0.1.4"
        ndkVersion = "29.0.14206865"
    }

    signingConfigs {
        if (keystorePropertiesFile.exists()) {
            create("release") {
                storeFile = rootProject.file(keystoreProperties.getProperty("storeFile"))
                storePassword = keystoreProperties.getProperty("storePassword")
                keyAlias = keystoreProperties.getProperty("keyAlias")
                keyPassword = keystoreProperties.getProperty("keyPassword")
            }
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            signingConfig = signingConfigs.findByName("release")
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

// Cross-compile bvox-core for the Android ABIs. cargo-ndk resolves the NDK
// linkers from ANDROID_HOME/ANDROID_NDK_HOME and copies each .so into
// <abi>/libbvox_core.so under rustJniLibs.
tasks.register<Exec>("buildRust") {
    workingDir(repoRoot)
    commandLine(
        "cargo", "ndk",
        "--platform", "23",
        *rustTargets.flatMap { listOf("-t", it) }.toTypedArray(),
        "-o", layout.buildDirectory.file("rustJniLibs").get().asFile.absolutePath,
        "build", "--release", "-p", "bvox-core",
    )
    inputs.dir(repoRoot.resolve("crates/bvox-core/src"))
    inputs.file(repoRoot.resolve("crates/bvox-core/Cargo.toml"))
    outputs.dir(layout.buildDirectory.file("rustJniLibs").get().asFile.absolutePath)
}

// Generate the UniFFI Kotlin bindings from the x86_64 cdylib (library mode
// reads the embedded metadata; no execution involved).
tasks.register<Exec>("generateUniffi") {
    dependsOn("buildRust")
    workingDir(repoRoot)
    val library = layout.buildDirectory.file("rustJniLibs/x86_64/libbvox_core.so").get().asFile
    commandLine(
        "cargo", "run", "--features", "uniffi-cli", "-p", "bvox-core",
        "--bin", "uniffi-bindgen",
        "generate",
        "--library", library.absolutePath,
        "--language", "kotlin",
        "--out-dir", layout.buildDirectory.file("generated/uniffi/kotlin").get().asFile.absolutePath,
        "--config", "uniffi.toml",
    )
    inputs.file(library.absolutePath)
    inputs.file(repoRoot.resolve("uniffi.toml"))
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

        // Branded artifact names (bvox-v<versionName>-<variant>.apk) instead of app-release.apk.
        variant.outputs.forEach { output ->
            val version = output.versionName.orNull ?: "0.0.0"
            output.outputFileName.set("bvox-v$version-${variant.name}.apk")
        }
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
    implementation(libs.androidx.compose.animation)
    implementation(libs.androidx.compose.material.icons)
    implementation(libs.androidx.media3.exoplayer)
    implementation(libs.androidx.media3.session)
    implementation(libs.androidx.media3.datasource)
    implementation(libs.jna) {
        artifact {
            type = "aar"
        }
    }
}
