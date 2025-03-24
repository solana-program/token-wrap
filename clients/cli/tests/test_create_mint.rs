use {
    crate::helpers::{execute_create_mint, setup_test_env, CreateMintResult},
    serial_test::serial,
    solana_program_pack::Pack,
    spl_token::{self, state::Mint as SplTokenMint},
    spl_token_2022::state::Mint as SplToken2022Mint,
    spl_token_wrap::{
        self, get_wrapped_mint_address, get_wrapped_mint_backpointer_address, state::Backpointer,
    },
};

mod helpers;

#[tokio::test]
#[serial]
async fn test_create_mint() {
    let env = setup_test_env().await;
    let CreateMintResult {
        wrapped_token_program,
        unwrapped_mint,
        ..
    } = execute_create_mint(&env).await;

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
    let wrapped_mint_data = SplToken2022Mint::unpack(&wrapped_mint_account.data).unwrap();
    assert_eq!(wrapped_mint_data.decimals, unwrapped_mint_data.decimals);

    // Verify backpointer data
    let backpointer = *bytemuck::from_bytes::<Backpointer>(&backpointer_account.data);
    assert_eq!(backpointer.unwrapped_mint, unwrapped_mint);
}
