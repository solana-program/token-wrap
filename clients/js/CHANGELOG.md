# @solana-program/token-wrap

## 2.2.0

### Minor Changes

- 4c60e2c: Update idl for new metadata sync instructions
- 7c4bc45: Bump deps to Kit 3.0

### Patch Changes

- 7c4bc45: Update CreateMint helper to support token-2022 extension sizing

## 2.1.0

### Minor Changes

- Generated methods for CloseStuckEscrow

## 2.0.0

### Major Changes

- Single signer helpers return instructions, not transactions

## 1.0.0

### Major Changes

- First stable release
  - expose createMintTx, singleSignerWrapTx, singleSignerUnwrapTx
  - provide multisig-helper builders and utilities (combinedMultisigTx, escrow creation, token-account helpers)
  - ship generated TypeScript types, codecs, PDA finders and error maps
  - add GitHub-Actions & Changesets configs for automated publishing
