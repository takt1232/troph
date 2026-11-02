plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
}

val appVersion = rootProject.file("../../VERSION").readText().trim()
val appVersionCode = appVersion
    .split('.')
    .fold(0) { acc, part -> acc * 1000 + part.toInt() }

android {
    namespace = "com.sixdaggers.pocketfiles"
    compileSdk = 36

    defaultConfig {
        applicationId = "com.sixdaggers.pocketfiles"
        minSdk = 30
        targetSdk = 35
        versionCode = appVersionCode
        versionName = appVersion
        ndk {
            abiFilters += "arm64-v8a"
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro",
            )
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    kotlinOptions {
        jvmTarget = "17"
    }

    buildFeatures {
        buildConfig = true
    }

    packaging {
        jniLibs {
            useLegacyPackaging = false
        }
    }

    lint {
        abortOnError = false
    }
}

dependencies {
    implementation("com.google.zxing:core:3.5.3")
}
