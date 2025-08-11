use {
    crate::helpers::{
        common::{init_mollusk, KeyedAccount, TokenProgram},
        extensions::{MintExtension, MintExtension::MetadataPointer as MetadataPointerExt},
        mint_builder::MintBuilder,
        sync_metadata_builder::SyncMetadataBuilder,
    },
    mollusk_svm::{program::create_program_account_loader_v3, result::Check},
    solana_account::Account,
    solana_instruction::{AccountMeta, Instruction},
    solana_program_error::ProgramError,
    solana_pubkey::Pubkey,
    spl_token_2022::{
        extension::{BaseStateWithExtensions, PodStateWithExtensions},
        pod::PodMint,
    },
    spl_token_metadata_interface::state::TokenMetadata,
    spl_token_wrap::{
        error::TokenWrapError, get_wrapped_mint_address, get_wrapped_mint_authority, id,
        instruction::TokenWrapInstruction,
    },
};

pub mod helpers;

#[test]
fn test_success_initialize_metadata() {
    let unwrapped_metadata = MintExtension::TokenMetadata {
        name: "Unwrapped Token".to_string(),
        symbol: "UWT".to_string(),
        uri: "https://unwrapped.dev/meta.json".to_string(),
        additional_metadata: vec![],
    };
    let unwrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken2022)
        .with_extension(unwrapped_metadata.clone())
        .build();

    let wrapped_mint_address = get_wrapped_mint_address(&unwrapped_mint.key, &spl_token_2022::id());
    let wrapped_mint_authority = get_wrapped_mint_authority(&wrapped_mint_address);
    let wrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken2022)
        .mint_key(wrapped_mint_address)
        .mint_authority(wrapped_mint_authority)
        .with_extension(MetadataPointerExt)
        .lamports(1_000_000_000)
        .build();

    let result = SyncMetadataBuilder::new()
        .unwrapped_mint(unwrapped_mint)
        .wrapped_mint(wrapped_mint.clone())
        .execute();

    let wrapped_mint_state =
        PodStateWithExtensions::<PodMint>::unpack(&result.wrapped_mint.account.data).unwrap();
    let wrapped_metadata = wrapped_mint_state
        .get_variable_len_extension::<TokenMetadata>()
        .unwrap();

    if let MintExtension::TokenMetadata {
        name,
        symbol,
        uri,
        additional_metadata,
    } = unwrapped_metadata
    {
        assert_eq!(wrapped_metadata.name, name);
        assert_eq!(wrapped_metadata.symbol, symbol);
        assert_eq!(wrapped_metadata.uri, uri);
        assert_eq!(wrapped_metadata.additional_metadata, additional_metadata);
        assert_eq!(
            Option::<Pubkey>::from(wrapped_metadata.update_authority).unwrap(),
            wrapped_mint_authority
        );
        assert_eq!(wrapped_metadata.mint, wrapped_mint.key);
    } else {
        panic!("destructure failed");
    }
}

#[test]
fn test_success_initialize_metadata_with_additional_fields() {
    let unwrapped_metadata = MintExtension::TokenMetadata {
        name: "Unwrapped Token".to_string(),
        symbol: "UWT".to_string(),
        uri: "https://unwrapped.dev/meta.json".to_string(),
        additional_metadata: vec![
            ("key1".to_string(), "value1".to_string()),
            ("key2".to_string(), "value2".to_string()),
        ],
    };
    let unwrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken2022)
        .with_extension(unwrapped_metadata.clone())
        .build();

    let wrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken2022)
        .mint_key(get_wrapped_mint_address(
            &unwrapped_mint.key,
            &spl_token_2022::id(),
        ))
        .mint_authority(get_wrapped_mint_authority(&get_wrapped_mint_address(
            &unwrapped_mint.key,
            &spl_token_2022::id(),
        )))
        .with_extension(MetadataPointerExt)
        .lamports(1_000_000_000)
        .build();

    let result = SyncMetadataBuilder::new()
        .unwrapped_mint(unwrapped_mint)
        .wrapped_mint(wrapped_mint.clone())
        .execute();

    let wrapped_mint_state =
        PodStateWithExtensions::<PodMint>::unpack(&result.wrapped_mint.account.data).unwrap();
    let wrapped_metadata = wrapped_mint_state
        .get_variable_len_extension::<TokenMetadata>()
        .unwrap();

    if let MintExtension::TokenMetadata {
        name,
        symbol,
        uri,
        additional_metadata,
    } = unwrapped_metadata
    {
        assert_eq!(wrapped_metadata.name, name);
        assert_eq!(wrapped_metadata.symbol, symbol);
        assert_eq!(wrapped_metadata.uri, uri);
        assert_eq!(wrapped_metadata.additional_metadata, additional_metadata);
        assert_eq!(
            Option::<Pubkey>::from(wrapped_metadata.update_authority).unwrap(),
            result.wrapped_mint_authority.key
        );
        assert_eq!(wrapped_metadata.mint, wrapped_mint.key);
    } else {
        panic!("destructure failed");
    }
}

#[test]
fn test_success_update_metadata() {
    // Wrapped mint with old metadata that needs to be updated
    let old_metadata = MintExtension::TokenMetadata {
        name: "Old Name".to_string(),
        symbol: "OLD".to_string(),
        uri: "https://old.uri".to_string(),
        additional_metadata: vec![],
    };

    // Unwrapped mint with the new metadata
    let new_metadata = MintExtension::TokenMetadata {
        name: "Updated Name".to_string(),
        symbol: "UPD".to_string(),
        uri: "https://updated.uri".to_string(),
        additional_metadata: vec![],
    };

    let unwrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken2022)
        .with_extension(new_metadata.clone())
        .build();

    let wrapped_mint_address = get_wrapped_mint_address(&unwrapped_mint.key, &spl_token_2022::id());
    let wrapped_mint_authority = get_wrapped_mint_authority(&wrapped_mint_address);
    let wrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken2022)
        .mint_key(wrapped_mint_address)
        .mint_authority(wrapped_mint_authority)
        .with_extension(MetadataPointerExt)
        .with_extension(old_metadata)
        .lamports(1_000_000_000)
        .build();

    let result = SyncMetadataBuilder::new()
        .unwrapped_mint(unwrapped_mint)
        .wrapped_mint(wrapped_mint.clone())
        .execute();

    let wrapped_mint_state =
        PodStateWithExtensions::<PodMint>::unpack(&result.wrapped_mint.account.data).unwrap();
    let wrapped_metadata = wrapped_mint_state
        .get_variable_len_extension::<TokenMetadata>()
        .unwrap();

    if let MintExtension::TokenMetadata {
        name,
        symbol,
        uri,
        additional_metadata,
    } = new_metadata
    {
        assert_eq!(wrapped_metadata.name, name);
        assert_eq!(wrapped_metadata.symbol, symbol);
        assert_eq!(wrapped_metadata.uri, uri);
        assert_eq!(wrapped_metadata.additional_metadata, additional_metadata);
        assert_eq!(
            Option::<Pubkey>::from(wrapped_metadata.update_authority).unwrap(),
            result.wrapped_mint_authority.key
        );
        assert_eq!(wrapped_metadata.mint, wrapped_mint.key);
    } else {
        panic!("destructure failed");
    }
}

#[test]
fn test_success_update_metadata_with_additional_fields() {
    let old_metadata = MintExtension::TokenMetadata {
        name: "Old Name".to_string(),
        symbol: "SYM".to_string(),
        uri: "uri".to_string(),
        additional_metadata: vec![
            ("key1".to_string(), "old_value".to_string()),
            ("key2".to_string(), "value2".to_string()), // to be removed
        ],
    };

    let new_metadata = MintExtension::TokenMetadata {
        name: "Updated Name".to_string(), // updated
        symbol: "SYM".to_string(),        // same
        uri: "uri".to_string(),           // same
        additional_metadata: vec![
            ("key1".to_string(), "new_value".to_string()), // updated
            ("key3".to_string(), "value3".to_string()),    // new
        ],
    };
    let unwrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken2022)
        .with_extension(new_metadata.clone())
        .build();

    let wrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken2022)
        .mint_key(get_wrapped_mint_address(
            &unwrapped_mint.key,
            &spl_token_2022::id(),
        ))
        .with_extension(MetadataPointerExt)
        .with_extension(old_metadata)
        .lamports(1_000_000_000)
        .build();

    let result = SyncMetadataBuilder::new()
        .unwrapped_mint(unwrapped_mint)
        .wrapped_mint(wrapped_mint.clone())
        .execute();

    let wrapped_mint_state =
        PodStateWithExtensions::<PodMint>::unpack(&result.wrapped_mint.account.data).unwrap();
    let wrapped_metadata = wrapped_mint_state
        .get_variable_len_extension::<TokenMetadata>()
        .unwrap();

    if let MintExtension::TokenMetadata {
        name,
        symbol,
        uri,
        additional_metadata,
    } = new_metadata
    {
        assert_eq!(wrapped_metadata.name, name);
        assert_eq!(wrapped_metadata.symbol, symbol);
        assert_eq!(wrapped_metadata.uri, uri);
        assert_eq!(wrapped_metadata.additional_metadata, additional_metadata);
        assert_eq!(
            Option::<Pubkey>::from(wrapped_metadata.update_authority).unwrap(),
            result.wrapped_mint_authority.key
        );
        assert_eq!(wrapped_metadata.mint, wrapped_mint.key);
    } else {
        panic!("destructure failed");
    }
}

#[test]
fn test_fail_unwrapped_mint_has_no_metadata() {
    let unwrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken2022)
        .build(); // No metadata extension

    SyncMetadataBuilder::new()
        .unwrapped_mint(unwrapped_mint)
        .check(Check::err(
            TokenWrapError::UnwrappedMintHasNoMetadata.into(),
        ))
        .execute();
}

#[test]
fn test_fail_incorrect_token_program() {
    let mollusk = init_mollusk();
    let unwrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken2022)
        .with_extension(MintExtension::TokenMetadata {
            name: "N".to_string(),
            symbol: "S".to_string(),
            uri: "U".to_string(),
            additional_metadata: vec![],
        })
        .build();
    let wrapped_mint_address = get_wrapped_mint_address(&unwrapped_mint.key, &spl_token_2022::id());
    let wrapped_mint_authority = get_wrapped_mint_authority(&wrapped_mint_address);
    let wrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken2022)
        .mint_key(wrapped_mint_address)
        .build();

    // Pass a fake program account instead of the real Token-2022 program
    let fake_program = KeyedAccount {
        key: Pubkey::new_unique(),
        account: create_program_account_loader_v3(&Pubkey::new_unique()),
    };

    let instruction = Instruction {
        program_id: id(),
        accounts: vec![
            AccountMeta::new(wrapped_mint.key, false),
            AccountMeta::new_readonly(wrapped_mint_authority, false),
            AccountMeta::new_readonly(unwrapped_mint.key, false),
            AccountMeta::new_readonly(fake_program.key, false),
        ],
        data: TokenWrapInstruction::SyncMetadataToToken2022.pack(),
    };

    let accounts = &[
        wrapped_mint.pair(),
        (wrapped_mint_authority, Account::default()),
        unwrapped_mint.pair(),
        fake_program.pair(),
    ];

    mollusk.process_and_validate_instruction(
        &instruction,
        accounts,
        &[Check::err(ProgramError::IncorrectProgramId)],
    );
}

// TODO: Remove test when spl-token supported
#[test]
fn test_fail_unwrapped_mint_not_token_2022() {
    let unwrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken)
        .build();

    SyncMetadataBuilder::new()
        .unwrapped_mint(unwrapped_mint)
        .check(Check::err(ProgramError::IncorrectProgramId))
        .execute();
}

#[test]
fn test_fail_wrapped_mint_not_token_2022() {
    let unwrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken2022)
        .with_extension(MintExtension::TokenMetadata {
            name: "N".to_string(),
            symbol: "S".to_string(),
            uri: "U".to_string(),
            additional_metadata: vec![],
        })
        .build();
    let wrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken) // Invalid program
        .mint_key(get_wrapped_mint_address(
            &unwrapped_mint.key,
            &spl_token_2022::id(),
        ))
        .build();

    SyncMetadataBuilder::new()
        .unwrapped_mint(unwrapped_mint)
        .wrapped_mint(wrapped_mint)
        .check(Check::err(ProgramError::IncorrectProgramId))
        .execute();
}

#[test]
fn test_fail_wrapped_mint_pda_mismatch() {
    let wrong_wrapped_mint = MintBuilder::new()
        .token_program(TokenProgram::SplToken2022)
        .mint_key(Pubkey::new_unique()) // Not the derived PDA
        .build();

    SyncMetadataBuilder::new()
        .wrapped_mint(wrong_wrapped_mint)
        .check(Check::err(TokenWrapError::WrappedMintMismatch.into()))
        .execute();
}

#[test]
fn test_fail_wrapped_mint_authority_pda_mismatch() {
    SyncMetadataBuilder::new()
        .wrapped_mint_authority(Pubkey::new_unique()) // Not the derived PDA
        .check(Check::err(TokenWrapError::MintAuthorityMismatch.into()))
        .execute();
}
