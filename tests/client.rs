use std::collections::BTreeMap;
use std::hash::Hasher;
use lazy_static::lazy_static;
use anyhow::Result;
use filecoin_proofs::{generate_fallback_sector_challenges, generate_single_vanilla_proof, PrivateReplicaInfo, PublicReplicaInfo, SECTOR_SIZE_2_KIB, SectorShape2KiB, TEST_SEED, WINDOW_POST_SECTOR_COUNT};
use storage_proofs_core::api_version::ApiVersion;
use std::sync::RwLock;
use storage_proofs_core::merkle::MerkleTreeTrait;
use storage_proofs_core::sector::SectorId;


fn do_window_post<Tree: 'static + MerkleTreeTrait>(
    sector_size: u64,
    total_sector_count: usize,
    sector_count: usize,
    fake: bool,
    api_version: ApiVersion,
) -> Result<()> {
    let mut rng = XorShiftRng::from_seed(TEST_SEED);

    let mut sectors = Vec::with_capacity(total_sector_count);
    let mut pub_replicas = BTreeMap::new();
    let mut priv_replicas = BTreeMap::new();

    let prover_fr: <Tree::Hasher as Hasher>::Domain = Fr::random(&mut rng).into();
    let mut prover_id = [0u8; 32];
    prover_id.copy_from_slice(AsRef::<[u8]>::as_ref(&prover_fr));

    let porep_id = match api_version {
        ApiVersion::V1_0_0 => ARBITRARY_POREP_ID_V1_0_0,
        ApiVersion::V1_1_0 => ARBITRARY_POREP_ID_V1_1_0,
    };

    for _ in 0..total_sector_count {
        let (sector_id, replica, comm_r, cache_dir) = if fake {
            create_fake_seal::<_, Tree>(&mut rng, sector_size, &porep_id, api_version)?
        } else {
            create_seal::<_, Tree>(
                &mut rng,
                sector_size,
                prover_id,
                true,
                &porep_id,
                api_version,
            )?
        };
        priv_replicas.insert(
            sector_id,
            PrivateReplicaInfo::new(replica.path().into(), comm_r, cache_dir.path().into())?,
        );
        pub_replicas.insert(sector_id, PublicReplicaInfo::new(comm_r)?);
        sectors.push((sector_id, replica, comm_r, cache_dir, prover_id));
    }
    assert_eq!(priv_replicas.len(), total_sector_count);
    assert_eq!(pub_replicas.len(), total_sector_count);
    assert_eq!(sectors.len(), total_sector_count);

    let random_fr: <Tree::Hasher as Hasher>::Domain = Fr::random(&mut rng).into();
    let mut randomness = [0u8; 32];
    randomness.copy_from_slice(AsRef::<[u8]>::as_ref(&random_fr));

    let replica_sectors = priv_replicas
        .iter()
        .map(|(sector, _replica)| *sector)
        .collect::<Vec<SectorId>>();

    let challenges = generate_fallback_sector_challenges::<Tree>(
        &config,
        &randomness,
        &replica_sectors,
        prover_id,
    )?;

    let mut vanilla_proofs = Vec::with_capacity(replica_sectors.len());

    for (sector_id, replica) in priv_replicas.iter() {
        let sector_challenges = &challenges[sector_id];
        let single_proof =
            generate_single_vanilla_proof::<Tree>(&config, *sector_id, replica, sector_challenges)?;

        vanilla_proofs.push(single_proof);
    }



    Ok(())
}

#[test]
#[ignore]
fn test_window_post_two_partitions_matching_2kib_base_8() -> Result<()> {
    let sector_size = SECTOR_SIZE_2_KIB;
    let sector_count = *WINDOW_POST_SECTOR_COUNT
        .read()
        .expect("WINDOW_POST_SECTOR_COUNT poisoned")
        .get(&sector_size)
        .expect("unknown sector size");

    window_post::<SectorShape2KiB>(
        sector_size,
        2 * sector_count,
        sector_count,
        false,
        ApiVersion::V1_0_0,
    )?;
    window_post::<SectorShape2KiB>(
        sector_size,
        2 * sector_count,
        sector_count,
        true,
        ApiVersion::V1_0_0,
    )?;
    window_post::<SectorShape2KiB>(
        sector_size,
        2 * sector_count,
        sector_count,
        false,
        ApiVersion::V1_1_0,
    )?;
    window_post::<SectorShape2KiB>(
        sector_size,
        2 * sector_count,
        sector_count,
        true,
        ApiVersion::V1_1_0,
    )
}

