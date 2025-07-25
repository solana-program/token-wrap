use {
    crate::helpers::{create_unwrapped_mint, execute_create_mint, setup_test_env},
    serial_test::serial,
    solana_program_pack::Pack,
    spl_token::{self, state::Mint as SplTokenMint},
    spl_token_2022::{
        extension::{
            confidential_transfer::ConfidentialTransferMint, BaseStateWithExtensions,
            PodStateWithExtensions,
        },
        pod::PodMint,
    },
    spl_token_wrap::{
        self, get_wrapped_mint_address, get_wrapped_mint_backpointer_address, state::Backpointer,
    },
};

mod helpers;

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_create_mint() {
    let env = setup_test_env().await;

    let unwrapped_token_program = spl_token::id();
    let wrapped_token_program = spl_token_2022::id();
    let unwrapped_mint = create_unwrapped_mint(&env, &unwrapped_token_program).await;
    execute_create_mint(&env, &unwrapped_mint, &wrapped_token_program).await;

    // Derive expected account addresses
    let wrapped_mint_address = get_wrapped_mint_address(&unwrapped_mint, &wrapped_token_program);
    let backpointer_address = get_wrapped_mint_backpointer_address(&wrapped_mint_address);

    // Fetch created accounts
    let wrapped_mint_account = env
        .rpc_client
        .get_account(&wrapped_mint_address)
        .await
        .unwrap();
    let backpointer_account = env
        .rpc_client
        .get_account(&backpointer_address)
        .await
        .unwrap();

    // Verify owners
    assert_eq!(wrapped_mint_account.owner, wrapped_token_program);
    assert_eq!(backpointer_account.owner, spl_token_wrap::id());

    // Verify mint properties
    let unwrapped_mint_account = env.rpc_client.get_account(&unwrapped_mint).await.unwrap();
    let unwrapped_mint_data = SplTokenMint::unpack(&unwrapped_mint_account.data).unwrap();
    let wrapped_mint_state =
        PodStateWithExtensions::<PodMint>::unpack(&wrapped_mint_account.data).unwrap();
    assert_eq!(
        wrapped_mint_state.base.decimals,
        unwrapped_mint_data.decimals
    );

    // Verify confidential transfer extension is present
    assert!(wrapped_mint_state
        .get_extension::<ConfidentialTransferMint>()
        .is_ok());
    assert_eq!(wrapped_mint_state.get_extension_types().unwrap().len(), 1);

    // Verify backpointer data
    let backpointer = *bytemuck::from_bytes::<Backpointer>(&backpointer_account.data);
    assert_eq!(backpointer.unwrapped_mint, unwrapped_mint);
}
