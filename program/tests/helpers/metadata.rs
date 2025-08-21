use {
    mpl_token_metadata::accounts::Metadata as MetaplexMetadata,
    spl_token_metadata_interface::state::TokenMetadata, std::collections::HashMap,
};

pub fn assert_metaplex_fields_synced(
    wrapped_metadata: &TokenMetadata,
    metaplex_metadata: &MetaplexMetadata,
) {
    let additional_meta_map: HashMap<_, _> = wrapped_metadata
        .additional_metadata
        .iter()
        .cloned()
        .collect();

    assert_eq!(
        additional_meta_map.get("seller_fee_basis_points").unwrap(),
        &metaplex_metadata.seller_fee_basis_points.to_string()
    );
    assert_eq!(
        additional_meta_map.get("primary_sale_happened").unwrap(),
        &metaplex_metadata.primary_sale_happened.to_string()
    );
    assert_eq!(
        additional_meta_map.get("is_mutable").unwrap(),
        &metaplex_metadata.is_mutable.to_string()
    );
    if let Some(edition_nonce) = metaplex_metadata.edition_nonce {
        assert_eq!(
            additional_meta_map.get("edition_nonce").unwrap(),
            &edition_nonce.to_string()
        );
    }
    if let Some(token_standard) = &metaplex_metadata.token_standard {
        assert_eq!(
            additional_meta_map.get("token_standard").unwrap(),
            &serde_json::to_string(token_standard).unwrap()
        );
    }
    if let Some(collection) = &metaplex_metadata.collection {
        assert_eq!(
            additional_meta_map.get("collection").unwrap(),
            &serde_json::to_string(collection).unwrap()
        );
    }
    if let Some(uses) = &metaplex_metadata.uses {
        assert_eq!(
            additional_meta_map.get("uses").unwrap(),
            &serde_json::to_string(uses).unwrap()
        );
    }
    if let Some(collection_details) = &metaplex_metadata.collection_details {
        assert_eq!(
            additional_meta_map.get("collection_details").unwrap(),
            &serde_json::to_string(collection_details).unwrap()
        );
    }
    if let Some(creators) = &metaplex_metadata.creators {
        if !creators.is_empty() {
            assert_eq!(
                additional_meta_map.get("creators").unwrap(),
                &serde_json::to_string(creators).unwrap()
            );
        }
    }
    if let Some(config) = &metaplex_metadata.programmable_config {
        assert_eq!(
            additional_meta_map.get("config").unwrap(),
            &serde_json::to_string(config).unwrap()
        );
    }
}
