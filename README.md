# SPL Token Wrap Program

[![GitHub Actions Workflow Status](https://img.shields.io/github/actions/workflow/status/solana-program/token-wrap/main.yml?logo=GitHub)](https://github.com/solana-program/token-wrap/actions/workflows/main.yml)

This program enables the creation of "wrapped" versions of existing SPL tokens, facilitating interoperability
between different token standards. If you are building a program and find yourself wishing you could take advantage of
some of the latest features of a specific token program, this might be for you!

## Features

* **Bidirectional Wrapping:** Convert tokens between SPL Token and SPL Token 2022 standards in either direction,
  including conversions between different SPL Token 2022 mints.
* **SPL Token 2022 Extension Support:**  Preserve or add SPL Token 2022 extensions (like transfer fees, confidential
  transfers, etc.) during the wrapping process.
* **Transfer Hook Compatibility:**  Integrates with tokens that implement the SPL Transfer Hook interface,
  enabling custom logic on token transfers.
* **Multisignature Support:** Compatible with multisig signers for both wrapping and unwrapping operations.

## How It Works

It supports three primary operations:

1. **`CreateMint`:** This operation initializes a new wrapped token mint and its associated backpointer account. Note,
   the caller must pre-fund this account with lamports. This is to avoid requiring writer+signer privileges on this
   instruction.

    * **Wrapped Mint:** An SPL Token or SPL Token 2022 mint account is created. The address of this mint is a
      PDA derived from the *unwrapped* token's mint address and the *wrapped* token program ID. This ensures a unique,
      deterministic relationship between the wrapped and unwrapped tokens. The wrapped mint's authority is also a PDA,
      controlled by the Token Wrap program.
    * **Backpointer:** An account (also a PDA, derived from the *wrapped* mint address) is created to store the
      address of the original *unwrapped* token mint. This allows anyone to easily determine the unwrapped token
      corresponding to a wrapped token, facilitating unwrapping.

2. **`Wrap`:**  This operation takes unwrapped tokens and mints wrapped tokens.

    * Unwrapped tokens are transferred from the user's account to an escrow account. The escrow account's owner is a PDA
      controlled by the Token Wrap program.
    * An equivalent amount of wrapped tokens is minted to the user's wrapped token account.

3. **`Unwrap`:** This operation burns wrapped tokens and releases unwrapped tokens.

    * Wrapped tokens are burned from the user's wrapped token account.
    * An equivalent amount of unwrapped tokens is transferred from the escrow account to the user's unwrapped token
      account.

The 1:1 relationship between wrapped and unwrapped tokens is maintained through the escrow mechanism, ensuring that
wrapped tokens are always fully backed by their unwrapped counterparts.

## Getting Started

### Prerequisites

1. **Install Solana CLI (v2.1.0 or later):**

   ```bash
   sh -c "$(curl -sSfL https://release.anza.xyz/v2.1.0/install)"
   ```

2. **Verify Installation:**

   ```bash
   solana --version
   ```
   Ensure the output shows version 2.1.0 or a compatible later version.

3. **Install `pnpm`:**

   This project uses `pnpm` for dependency management and scripting. If you don't have it installed:

     ```bash
      npm install -g pnpm
     ```
4. **Install Dependencies:**

    ```bash
    pnpm install
    ```

### Building and Testing

1. **Build the Program:**

   ```bash
   pnpm programs:build
   ```
   This compiles the Rust program into a Solana executable (`.so` file).

2. **Run Tests:**

   ```bash
   pnpm programs:test
   ```
   This executes the integration tests to verify the program's functionality.

## License

This project is licensed under the Apache License 2.0.