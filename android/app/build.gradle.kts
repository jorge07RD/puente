import java.util.Properties

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
}

// La clave vive fuera del proyecto: perderla obligaría a reinstalar la app.
// Sin ella, el APK se firma con la clave de depuración de cada máquina.
val signingDir = file(System.getProperty("user.home") + "/.local/share/puente/signing")
val signingFile = signingDir.resolve("signing.properties")
val signing = Properties().apply {
    if (signingFile.exists()) signingFile.inputStream().use { load(it) }
}

android {
    namespace = "dev.puente.app"
    compileSdk = 35

    defaultConfig {
        applicationId = "dev.puente.app"
        minSdk = 29
        targetSdk = 35
        versionCode = 6
        versionName = "0.3.1"
        ndk {
            abiFilters += "arm64-v8a"
        }
    }

    if (signingFile.exists()) {
        signingConfigs {
            create("puente") {
                storeFile = signingDir.resolve("puente.keystore")
                storePassword = signing.getProperty("storePassword")
                keyAlias = signing.getProperty("keyAlias")
                keyPassword = signing.getProperty("keyPassword")
            }
        }

        buildTypes {
            getByName("debug") {
                signingConfig = signingConfigs.getByName("puente")
            }
            getByName("release") {
                signingConfig = signingConfigs.getByName("puente")
            }
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    kotlinOptions {
        jvmTarget = "17"
    }
}

dependencies {
    // Lector de códigos QR de Google Play: trae su propia pantalla y no pide cámara.
    implementation("com.google.android.gms:play-services-code-scanner:16.1.0")
}
