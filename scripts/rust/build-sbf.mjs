#!/usr/bin/env zx
import 'zx/globals';
import {
    cliArguments,
    workingDirectory,
} from '../utils.mjs';

const [folder, ...args] = cliArguments();
const mainProgramManifestPath = path.join(workingDirectory, folder, 'Cargo.toml');
const testProgramManifestPath = path.join(workingDirectory, folder, 'tests', 'helpers', 'test-transfer-hook', 'Cargo.toml');

console.log(`Building main program: ${mainProgramManifestPath}`);
await $`cargo-build-sbf --manifest-path ${mainProgramManifestPath} ${args}`;

console.log(`Building test program: ${testProgramManifestPath}`);
await $`cargo-build-sbf --manifest-path ${testProgramManifestPath} ${args}`;
