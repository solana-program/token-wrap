# SPL Token Wrap Program

[![GitHub Actions Workflow Status](https://img.shields.io/github/actions/workflow/status/solana-program/token-wrap/main.yml?logo=GitHub)](https://github.com/solana-program/token-wrap/actions/workflows/main.yml)

This program enables the creation of "wrapped" versions of existing SPL tokens, facilitating interoperability
between different token standards. If you are building an app with a mint/token and find yourself wishing you could take
advantage of some of the latest features of a specific token program, this might be for you!

## Features

* **Bidirectional Wrapping:** Convert tokens between SPL Token and SPL Token 2022 standards in either direction,
  including conversions between different SPL Token 2022 mints.
* **Confidential Transfers by Default:** All wrapped tokens created under the Token-2022 standard automatically include
  the `ConfidentialTransferMint` extension, enabling the option for privacy-preserving transactions. This feature is
  immutable and requires no additional configuration.
    * Note: Any other extensions present on the original unwrapped mint are not preserved in the wrapped version. If you
      would like a different extensions config for wrapped Token-2022 mints, this requires forking and
      updating the `CreateMint` instruction.
* **Transfer Hook Compatibility:**  Integrates with tokens that implement the SPL Transfer Hook interface,
  enabling custom logic on token transfers.
* **Multisignature Support:** Compatible with multisig signers for both wrapping and unwrapping operations.

## How It Works

It supports four primary operations:

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

2. **`Wrap`:**  This operation accepts deposits of unwrapped tokens and mints wrapped tokens.

    * Unwrapped tokens are transferred from the user's account to an escrow account. Any unwrapped token account whose
      owner is a PDA controlled by the Token Wrap program can be used.
    * An equivalent amount of wrapped tokens is minted to the user's wrapped token account.

3. **`Unwrap`:** This operation burns wrapped tokens and releases unwrapped token deposits.

    * Wrapped tokens are burned from the user's wrapped token account.
    * An equivalent amount of unwrapped tokens is transferred from the escrow account to the user's unwrapped token
      account.

4. **`CloseStuckEscrow`:** This operation handles an edge case with re-creating a mint with the MintCloseAuthority
   extension.

    * The escrow ATA can get "stuck" when an unwrapped mint with a close authority is closed and then a new mint is
      created at the same address but with different extensions, leaving the escrow ATA (Associated Token Account) in an
      incompatible state.
    * The instruction closes the old escrow ATA and returns the lamports to a specified destination account.
    * This operation will only succeed if the current escrow has zero balance and has different extensions than the
      mint.
    * After closing the stuck escrow, the client is responsible for recreating the ATA with the correct extensions.

The 1:1 relationship between wrapped and unwrapped tokens is maintained through the escrow mechanism, ensuring that
wrapped tokens are always fully backed by their unwrapped counterparts.

## Permissionless design

The SPL Token Wrap program is designed to be **permissionless**. This means:

* **Anyone can create a wrapped mint:**  No special permissions or whitelisting is required to create a wrapped
  version of an existing mint. The `CreateMint` instruction is open to all users, provided they can
  pay the required rent for the new accounts.
* **Anyone can wrap and unwrap tokens:**  Once a wrapped mint has been created, any user holding the underlying
  unwrapped tokens can use the `Wrap` and `Unwrap` instructions. All transfers are controlled by PDAs owned by the Token
  Wrap program itself. However, it is important to note that if the *unwrapped* token has a freeze authority,
  that freeze authority is *preserved* in the wrapped token.

## Confidential Transfer extension

The `ConfidentialTransferMint` extension is added to every Token-2022 wrapped mint and initialized with the following
config:

* **No Authority:** The confidential transfer authority is set to `None`, making the configuration immutable. This
  ensures that the privacy features cannot be disabled or altered after the wrapped mint is created.
* **No Auditor:** The wrapped mints are created without a confidential transfer auditor. This means that there is no
  third party that can view the details of confidential transactions.
* **Automatic Account Approval:** New token accounts are approved for confidential transfers by default. This allows
  users to make private transactions permissionlessly.

## Audits

| Auditor              | Date       | Version                                                                                               | Report                                                                                                                                                |
|----------------------|------------|-------------------------------------------------------------------------------------------------------|-------------------------------------------------------------------------------------------------------------------------------------------------------|
| Zellic               | 2025-05-16 | [75c5529](https://github.com/solana-program/token-wrap/tree/75c5529d5a191f12bd58b6b92ca0104ce3464763) | [PDF](https://github.com/anza-xyz/security-audits/blob/2294fc0e61c153c8aed174e9f63a1730683f1f2a/spl/ZellicTokenWrapAudit-2025-05-16.pdf)              |
| Runtime Verification | 2025-06-11 | [dd71fc1](https://github.com/solana-program/token-wrap/tree/dd71fc10c651b07b7d62b151021216e5321b1789) | [PDF](https://github.com/anza-xyz/security-audits/blob/2294fc0e61c153c8aed174e9f63a1730683f1f2a/spl/RuntimeVerificationTokenWrapAudit-2025-06-11.pdf) |

## Getting Started

### Prerequisites

1. Install [Solana CLI](https://docs.anza.xyz/cli/install)
    - Ensure version matches [the crate manifest](./Cargo.toml).
2. Install [pnpm](https://pnpm.io/installation)
3. Install project dependencies:

    ```bash
    pnpm install
    ```

### Building and Testing

1. **Build the Program:**

   ```bash
   pnpm programs:build
   ```

2. **Run Tests:**

   ```bash
   pnpm programs:test
   ```

## License

This project is licensed under the Apache License 2.0.