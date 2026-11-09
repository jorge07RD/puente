# Product

<!-- impeccable:product-schema 1 -->

## Platform

android

## Users

The first user is the author, working at a Linux desktop (Omarchy/Hyprland) with an Android phone nearby. The project will be published as open source on GitHub, so other Linux and Android users must be able to install it, pair it and understand it without help from the author.

The job: keep the phone within reach from the PC during normal work, viewing and operating it, and seeing what it needs, without picking it up or connecting a cable.

## Product Purpose

Puente brings together what KDE Connect and scrcpy do separately: phone screen and control, notifications, clipboard, files, battery, media control, ringing the phone and phone audio on the PC. Success is that the phone is usable from the PC after one pairing, with no USB, developer mode or ADB.

## Positioning

One codebase in Rust, one app on each side and one pairing, instead of two tools with separate setup. It works without ADB by using Android's own screen capture and Accessibility services.

## Operating Context

- Used alongside other windows on a Linux desktop, often for short interactions while working.
- PC and phone are on the same local network. Traffic is end-to-end TLS between paired devices.
- On the phone the app is mostly opened to pair, grant permissions and start sharing.

## Capabilities and Constraints

- Built with Rust and egui on both Linux desktop and Android, with a thin Kotlin layer for classes Android requires. The interface is native, not web.
- Android requires a confirmation each time screen sharing starts and a one-time Accessibility permission, which is flagged as a restricted setting on sideloaded apps.
- Apps that block capture appear black.
- Clipboard from phone to PC cannot be automatic, because Android blocks background reads.
- Text typed from the PC goes through Accessibility and can fail in games or custom-drawn fields.
- The phone only obeys input while its screen is being shared.

## Brand Commitments

- Name: Puente (provisional, Spanish for "bridge"). Interface copy is in Spanish.
- Visual reference chosen by the author: a dark grid of instrument-like cards with dot-matrix graphics, numbered monospaced labels, large thin figures, near-black panels, white dotted data and one fixed red-orange accent.

## Evidence on Hand

Working phone mirroring, control, pairing and data channels tested on a Samsung SM-S918U. There are no users, testimonials, benchmarks or published releases yet; none may be claimed.

## Product Principles

- The phone's screen is the main subject whenever it is being shared.
- Every number shown is measured from the real connection or phone. Nothing is decorative data.
- Android's required permission steps are explained plainly, never hidden.
- A stranger who just installed it must be able to reach a working pairing.
