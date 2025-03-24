#!/usr/bin/env zx
import 'zx/globals';
import { cliArguments, workingDirectory } from '../utils.mjs';

const [folder, ...formatArgs] = cliArguments();

// Format the client using Prettier.
cd(path.join(workingDirectory, folder));
await $`pnpm install`;
await $`pnpm format ${formatArgs}`;
