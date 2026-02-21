# conformal-scripts Reference

This is a CLI entry point for various build-related scripts related to the conformal audio framework.

## `bootstrap`

Make sure all build requirements are installed

Usage: `bun x conformal-scripts bootstrap [options]`

### `--rust-version`

This option is **required**

`--rust-version <version>`

The version of Rust to use (default: 1.93.1)

### `--cargo-about-version`

This option is **required**

`--cargo-about-version <version>`

The version of cargo-about to use (default: 0.6.6)

### `--vst-sdk-version`

This option is **required**

`--vst-sdk-version <version>`

The version of the VST SDK to use (default: v3.8.0_build_66)

### `--with-miri`

`--with-miri`

Install the Rust nightly toolchain with [miri](https://github.com/rust-lang/miri) undefined behavior checker. This can optionally be used with the `ci` command to run tests with miri.

### `--with-vst-sdk`

`--with-vst-sdk`

Install the VST SDK to allow validating VST plug-ins

## `bootstrap-rust-toolchain`

Install only the Rust toolchain

Usage: `bun x conformal-scripts bootstrap-rust-toolchain [options]`

### `--rust-version`

This option is **required**

`--rust-version <version>`

The version of Rust to use (default: 1.93.1)

## `check-lfs`

Checks that no files are checked in that should be lfs tracked

Usage: `bun x conformal-scripts check-lfs [options]`

## `check-todo`

Check if any rust files contain TODOs

Usage: `bun x conformal-scripts check-todo [options]`

## `check-format`

Check if the code is formatted correctly

Usage: `bun x conformal-scripts check-format [options]`

## `format`

Auto-format code

Usage: `bun x conformal-scripts format [options]`

## `package`

Package a plug-in

Usage: `bun x conformal-scripts package [options]`

### `--dist`

`-d, --dist`

Whether to create a distributable package, including an installer

### `--release`

`--release`

Build with optimizations

## `validate`

Validate a plug-in using the Steinberg validator

Usage: `bun x conformal-scripts validate [options]`

### `--release`

`--release`

Build with optimizations

## `cargo`

Runs cargo

Usage: `bun x conformal-scripts cargo [options] [args...]`

## `ci`

Run a full CI pass

Usage: `bun x conformal-scripts ci [options]`

### `--with-miri`

`--with-miri`

Run tests with [miri](https://github.com/rust-lang/miri) undefined behavior checks

## `web-script`

Run a script defined in a specific web-package. If no package is provided, it will run on all packages that define the script. The package, if provided, must be the first argument after the script name.

Usage: `bun x conformal-scripts web-script [options] [args...]`

### `--script`

This option is **required**

`-s, --script <script>`

The script to run

## `create-plugin`

Create a new plug-in from a template

Usage: `bun x conformal-scripts create-plugin [options]`

### `--plug_type`

This option is **required**

`--plug_type <plug_type>`

The type of plug-in to create ('effect' or 'synth')

### `--plug_slug`

This option is **required**

`--plug_slug <plug_slug>`

The name of the first plug-in in lower snake_case, e.g. `my_plugin`

### `--vendor_name`

This option is **required**

`--vendor_name <vendor_name>`

Human-readable vendor name, e.g. `My Project`. DAWs often present plug-ins grouped by vendor

### `--plug_name`

This option is **required**

`--plug_name <plug_name>`

Human-readable vendor name, e.g. `My Plug-in`

## `check-licenses`

Check if all rust dependency licenses are valid

Usage: `bun x conformal-scripts check-licenses [options]`

## `dev-mode`

Turn the dev mode preference on or off for a plug-in

Usage: `bun x conformal-scripts dev-mode [options]`

### `--on`

`--on`

Enable dev mode

### `--off`

`--off`

Disable dev mode

