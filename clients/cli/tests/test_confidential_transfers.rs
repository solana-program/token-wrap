use {
    crate::helpers::{create_unwrapped_mint, execute_create_mint, setup_test_env},
    serial_test::serial,
    spl_token_2022::{
        extension::{
            confidential_transfer::ConfidentialTransferMint, BaseStateWithExtensions,
            PodStateWithExtensions,
        },
        pod::PodMint,
    },
    std::process::Command,
};

mod helpers;

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_confidential_transfer_with_wrap_and_deposit() {
    let env = setup_test_env().await;
    let unwrapped_token_program = spl_token_2022::id();
    let wrapped_token_program = spl_token_2022::id();
    let unwrapped_mint = create_unwrapped_mint(&env, &unwrapped_token_program).await;

    execute_create_mint(&env, &unwrapped_mint, &wrapped_token_program).await;
    let wrapped_mint_address =
        spl_token_wrap::get_wrapped_mint_address(&unwrapped_mint, &wrapped_token_program);

    // Verify the wrapped mint's confidential transfer configuration
    let wrapped_mint_account = env
        .rpc_client
        .get_account(&wrapped_mint_address)
        .await
        .unwrap();
    let wrapped_mint_state =
        PodStateWithExtensions::<PodMint>::unpack(&wrapped_mint_account.data).unwrap();
    let ct_mint = wrapped_mint_state
        .get_extension::<ConfidentialTransferMint>()
        .unwrap();

    assert_eq!(ct_mint.authority, Default::default());
    assert!(bool::from(ct_mint.auto_approve_new_accounts));
    assert_eq!(ct_mint.auditor_elgamal_pubkey, Default::default());

    // Create a ATA for the new wrapped mint
    let create_status = Command::new("spl-token")
        .args([
            "--config",
            &env.config_file_path,
            "create-account",
            &wrapped_mint_address.to_string(),
        ])
        .status()
        .unwrap();
    assert!(create_status.success());

    // Configure ATA for confidential transfers to verify confidential transfer
    // extension working properly
    let config_status = Command::new("spl-token")
        .args([
            "--config",
            &env.config_file_path,
            "configure-confidential-transfer-account",
            &wrapped_mint_address.to_string(),
        ])
        .status()
        .unwrap();
    assert!(config_status.success());
}
