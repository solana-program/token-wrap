#!/usr/bin/env zx
import 'zx/globals';
import {
    cliArguments,
    workingDirectory,
} from '../utils.mjs';

const [folder, ...args] = cliArguments();
const mainProgramManifestPath = path.join(workingDirectory, folder, 'Cargo.toml');
const testProgramManifestPath = path.join(workingDirectory, folder, 'tests', 'helpers', 'test-transfer-hook', 'Cargo.toml');

await $`RUST_LOG=error cargo test-sbf --manifest-path ${testProgramManifestPath} ${args}`;
await $`RUST_LOG=error cargo test-sbf --manifest-path ${mainProgramManifestPath} ${args}`;
