use {
    crate::common::{
        helpers::setup_test_env, test_close_stuck_escrow::*, test_confidential_transfers::*,
        test_create_escrow_account::*, test_create_mint::*, test_pdas::*,
        test_sync_metadata_to_spl_token::*, test_sync_metadata_to_token2022::*, test_unwrap::*,
        test_wrap::*,
    },
    libtest_mimic::{Arguments, Trial},
};

mod common;

#[macro_export]
macro_rules! async_trial {
    ($test_func:ident, $env:ident) => {{
        let test_env = $env.clone();
        let handle = tokio::runtime::Handle::current();
        Trial::test(stringify!($test_func), move || {
            handle.block_on(async { $test_func(&test_env).await });
            Ok(())
        })
    }};
}

#[tokio::main]
async fn main() {
    let args = Arguments::from_args();
    let env = setup_test_env().await;

    // maybe come up with a way to do this through a some macro tag on the function?
    let tests = vec![
        async_trial!(test_only_token_2022_allowed, env),
        async_trial!(test_create_mint_close_stuck_escrow_fails, env),
        async_trial!(test_successful_close, env),
        async_trial!(test_confidential_transfer_with_wrap_and_deposit, env),
        async_trial!(test_create_ata_escrow_account_for_spl_token_mint, env),
        async_trial!(test_create_ata_escrow_account_for_token2022_mint, env),
        async_trial!(test_create_escrow_account_with_signer, env),
        async_trial!(test_create_escrow_account_signer_idempotent, env),
        async_trial!(test_create_escrow_account_ata_idempotent, env),
        async_trial!(test_create_escrow_account_with_wrong_mint_owner, env),
        async_trial!(test_create_escrow_account_with_wrong_account_type, env),
        async_trial!(test_create_mint, env),
        async_trial!(test_pdas, env),
        async_trial!(test_sync_metadata_from_token2022_to_spl_token, env),
        async_trial!(test_sync_metadata_from_spl_token_to_spl_token, env),
        async_trial!(test_sync_metadata_from_spl_token_to_token2022, env),
        async_trial!(test_sync_from_token2022_with_self_referential_pointer, env),
        async_trial!(test_sync_from_token2022_with_external_metaplex_pointer, env),
        async_trial!(test_sync_from_token2022_without_pointer_fallback, env),
        async_trial!(test_fail_sync_from_invalid_mint_owner, env),
        async_trial!(test_unwrap_single_signer_with_defaults, env),
        async_trial!(test_unwrap_single_signer_with_optional_flags, env),
        async_trial!(test_unwrap_fail_invalid_wrapped_token_program, env),
        async_trial!(test_unwrap_fail_mismatched_unwrapped_mint, env),
        async_trial!(test_unwrap_fail_invalid_unwrapped_token_program, env),
        async_trial!(test_unwrap_with_multisig, env),
        async_trial!(test_wrap_single_signer_with_defaults, env),
        async_trial!(test_wrap_single_signer_with_optional_flags, env),
        async_trial!(test_wrap_with_multisig, env),
    ];

    libtest_mimic::run(&args, tests).exit();
}
