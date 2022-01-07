use std::collections::BTreeMap;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
// use rand::{random, Rng, SeedableRng};
use filecoin_hashers::Hasher;
use lazy_static::lazy_static;
use anyhow::{ensure, Result};
use filecoin_proofs::{add_piece, as_safe_commitment, clear_cache, Commitment, compute_comm_d, fauxrep_aux, generate_fallback_sector_challenges, generate_piece_commitment, generate_single_vanilla_proof, get_seal_inputs, PaddedBytesAmount, PieceInfo, POREP_PARTITIONS, PoRepConfig, PoRepProofPartitions, PoStConfig, PoStType, PrivateReplicaInfo, ProverId, PublicReplicaInfo, seal_commit_phase1, seal_commit_phase2, seal_pre_commit_phase1, seal_pre_commit_phase2, SealCommitOutput, SealPreCommitOutput, SealPreCommitPhase1Output, SECTOR_SIZE_2_KIB, SectorShape2KiB, SectorSize, TEST_SEED, UnpaddedByteIndex, UnpaddedBytesAmount, unseal_range, validate_cache_for_commit, validate_cache_for_precommit_phase2, verify_seal, WINDOW_POST_CHALLENGE_COUNT, WINDOW_POST_SECTOR_COUNT};
use storage_proofs_core::api_version::ApiVersion;
use std::sync::RwLock;
use std::time::Duration;
use rand_xorshift::XorShiftRng;
use storage_proofs_core::error::Error::Serde;
use storage_proofs_core::merkle::MerkleTreeTrait;
use storage_proofs_core::sector::SectorId;
use storage_proofs_post::fallback;
use tokio::runtime::Runtime;
use tonic::Request;
use blstrs::Scalar as Fr;
use ff::Field;
use rand::{random, Rng, SeedableRng};
use storage_proofs_post::fallback::FallbackPoStCompound;
use uuid::Uuid;
use window_post_snark_server::client::new_client;
use window_post_snark_server::snark_proof_grpc::{GetTaskResultRequest, GetWorkerStatusRequest, SnarkTaskRequestParams};
use tempfile::{tempdir, NamedTempFile, TempDir};

const ARBITRARY_POREP_ID_V1_0_0: [u8; 32] = [127; 32];
const ARBITRARY_POREP_ID_V1_1_0: [u8; 32] = [128; 32];

fn porep_config(sector_size: u64, porep_id: [u8; 32], api_version: ApiVersion) -> PoRepConfig {
    PoRepConfig {
        sector_size: SectorSize(sector_size),
        partitions: PoRepProofPartitions(
            *POREP_PARTITIONS
                .read()
                .expect("POREP_PARTITIONS poisoned")
                .get(&sector_size)
                .expect("unknown sector size"),
        ),
        porep_id,
        api_version,
    }
}

fn create_fake_seal<R: rand::Rng, Tree: 'static + MerkleTreeTrait>(
    mut rng: &mut R,
    sector_size: u64,
    porep_id: &[u8; 32],
    api_version: ApiVersion,
) -> Result<(SectorId, NamedTempFile, Commitment, TempDir)> {
    fil_logger::init();

    let sealed_sector_file = NamedTempFile::new()?;

    let config = porep_config(sector_size, *porep_id, api_version);

    let cache_dir = tempdir().unwrap();

    let sector_id = rng.gen::<u64>().into();

    let comm_r = fauxrep_aux::<_, _, _, Tree>(
        &mut rng,
        config,
        cache_dir.path(),
        sealed_sector_file.path(),
    )?;

    Ok((sector_id, sealed_sector_file, comm_r, cache_dir))
}

fn run_seal_pre_commit_phase1<Tree: 'static + MerkleTreeTrait>(
    config: PoRepConfig,
    prover_id: ProverId,
    sector_id: SectorId,
    ticket: [u8; 32],
    cache_dir: &TempDir,
    mut piece_file: &mut NamedTempFile,
    sealed_sector_file: &NamedTempFile,
) -> Result<(Vec<PieceInfo>, SealPreCommitPhase1Output<Tree>)> {
    let number_of_bytes_in_piece =
        UnpaddedBytesAmount::from(PaddedBytesAmount(config.sector_size.into()));

    let piece_info = generate_piece_commitment(piece_file.as_file_mut(), number_of_bytes_in_piece)?;
    piece_file.as_file_mut().seek(SeekFrom::Start(0))?;

    let mut staged_sector_file = NamedTempFile::new()?;
    add_piece(
        &mut piece_file,
        &mut staged_sector_file,
        number_of_bytes_in_piece,
        &[],
    )?;

    let piece_infos = vec![piece_info];

    let phase1_output = seal_pre_commit_phase1::<_, _, _, Tree>(
        config,
        cache_dir.path(),
        staged_sector_file.path(),
        sealed_sector_file.path(),
        prover_id,
        sector_id,
        ticket,
        &piece_infos,
    )?;

    validate_cache_for_precommit_phase2(
        cache_dir.path(),
        staged_sector_file.path(),
        &phase1_output,
    )?;

    Ok((piece_infos, phase1_output))
}

#[allow(clippy::too_many_arguments)]
fn generate_proof<Tree: 'static + MerkleTreeTrait>(
    config: PoRepConfig,
    cache_dir_path: &Path,
    sealed_sector_file: &NamedTempFile,
    prover_id: ProverId,
    sector_id: SectorId,
    ticket: [u8; 32],
    seed: [u8; 32],
    pre_commit_output: &SealPreCommitOutput,
    piece_infos: &[PieceInfo],
) -> Result<(SealCommitOutput, Vec<Vec<Fr>>, [u8; 32], [u8; 32])> {
    let phase1_output = seal_commit_phase1::<_, Tree>(
        config,
        cache_dir_path,
        sealed_sector_file.path(),
        prover_id,
        sector_id,
        ticket,
        seed,
        pre_commit_output.clone(),
        piece_infos,
    )?;

    clear_cache::<Tree>(cache_dir_path)?;

    ensure!(
        seed == phase1_output.seed,
        "seed and phase1 output seed do not match"
    );
    ensure!(
        ticket == phase1_output.ticket,
        "seed and phase1 output ticket do not match"
    );

    let comm_r = phase1_output.comm_r;
    let inputs = get_seal_inputs::<Tree>(
        config,
        phase1_output.comm_r,
        phase1_output.comm_d,
        prover_id,
        sector_id,
        phase1_output.ticket,
        phase1_output.seed,
    )?;
    let result = seal_commit_phase2(config, phase1_output, prover_id, sector_id)?;

    Ok((result, inputs, seed, comm_r))
}

#[allow(clippy::too_many_arguments)]
fn unseal<Tree: 'static + MerkleTreeTrait>(
    config: PoRepConfig,
    cache_dir_path: &Path,
    sealed_sector_file: &NamedTempFile,
    prover_id: ProverId,
    sector_id: SectorId,
    ticket: [u8; 32],
    seed: [u8; 32],
    pre_commit_output: &SealPreCommitOutput,
    piece_infos: &[PieceInfo],
    piece_bytes: &[u8],
    commit_output: &SealCommitOutput,
) -> Result<()> {
    let comm_d = pre_commit_output.comm_d;
    let comm_r = pre_commit_output.comm_r;

    let mut unseal_file = NamedTempFile::new()?;
    let _ = unseal_range::<_, _, _, Tree>(
        config,
        cache_dir_path,
        sealed_sector_file,
        &unseal_file,
        prover_id,
        sector_id,
        comm_d,
        ticket,
        UnpaddedByteIndex(508),
        UnpaddedBytesAmount(508),
    )?;

    unseal_file.seek(SeekFrom::Start(0))?;

    let mut contents = vec![];
    assert!(
        unseal_file.read_to_end(&mut contents).is_ok(),
        "failed to populate buffer with unsealed bytes"
    );
    assert_eq!(contents.len(), 508);
    assert_eq!(&piece_bytes[508..508 + 508], &contents[..]);

    let computed_comm_d = compute_comm_d(config.sector_size, piece_infos)?;

    assert_eq!(
        comm_d, computed_comm_d,
        "Computed and expected comm_d don't match."
    );

    let verified = verify_seal::<Tree>(
        config,
        comm_r,
        comm_d,
        prover_id,
        sector_id,
        ticket,
        seed,
        &commit_output.proof,
    )?;
    assert!(verified, "failed to verify valid seal");
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn proof_and_unseal<Tree: 'static + MerkleTreeTrait>(
    config: PoRepConfig,
    cache_dir_path: &Path,
    sealed_sector_file: &NamedTempFile,
    prover_id: ProverId,
    sector_id: SectorId,
    ticket: [u8; 32],
    seed: [u8; 32],
    pre_commit_output: SealPreCommitOutput,
    piece_infos: &[PieceInfo],
    piece_bytes: &[u8],
) -> Result<()> {
    let (commit_output, _commit_inputs, _seed, _comm_r) = generate_proof::<Tree>(
        config,
        cache_dir_path,
        sealed_sector_file,
        prover_id,
        sector_id,
        ticket,
        seed,
        &pre_commit_output,
        piece_infos,
    )?;

    unseal::<Tree>(
        config,
        cache_dir_path,
        sealed_sector_file,
        prover_id,
        sector_id,
        ticket,
        seed,
        &pre_commit_output,
        piece_infos,
        piece_bytes,
        &commit_output,
    )
}


fn create_seal<R: Rng, Tree: 'static + MerkleTreeTrait>(
    rng: &mut R,
    sector_size: u64,
    prover_id: ProverId,
    skip_proof: bool,
    porep_id: &[u8; 32],
    api_version: ApiVersion,
) -> Result<(SectorId, NamedTempFile, Commitment, TempDir)> {
    fil_logger::init();

    let (mut piece_file, piece_bytes) = generate_piece_file(sector_size)?;
    let sealed_sector_file = NamedTempFile::new()?;
    let cache_dir = tempdir().expect("failed to create temp dir");

    let config = porep_config(sector_size, *porep_id, api_version);
    let ticket = rng.gen();
    let seed = rng.gen();
    let sector_id = rng.gen::<u64>().into();

    let (piece_infos, phase1_output) = run_seal_pre_commit_phase1::<Tree>(
        config,
        prover_id,
        sector_id,
        ticket,
        &cache_dir,
        &mut piece_file,
        &sealed_sector_file,
    )?;

    let pre_commit_output = seal_pre_commit_phase2(
        config,
        phase1_output,
        cache_dir.path(),
        sealed_sector_file.path(),
    )?;

    let comm_r = pre_commit_output.comm_r;

    validate_cache_for_commit::<_, _, Tree>(cache_dir.path(), sealed_sector_file.path())?;

    if skip_proof {
        clear_cache::<Tree>(cache_dir.path())?;
    } else {
        proof_and_unseal::<Tree>(
            config,
            cache_dir.path(),
            &sealed_sector_file,
            prover_id,
            sector_id,
            ticket,
            seed,
            pre_commit_output,
            &piece_infos,
            &piece_bytes,
        )
            .expect("failed to proof_and_unseal");
    }

    Ok((sector_id, sealed_sector_file, comm_r, cache_dir))
}


fn generate_piece_file(sector_size: u64) -> Result<(NamedTempFile, Vec<u8>)> {
    let number_of_bytes_in_piece = UnpaddedBytesAmount::from(PaddedBytesAmount(sector_size));

    let piece_bytes: Vec<u8> = (0..number_of_bytes_in_piece.0)
        .map(|_| random::<u8>())
        .collect();

    let mut piece_file = NamedTempFile::new()?;
    piece_file.write_all(&piece_bytes)?;
    piece_file.as_file_mut().sync_all()?;
    piece_file.as_file_mut().seek(SeekFrom::Start(0))?;

    Ok((piece_file, piece_bytes))
}



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

    let config = PoStConfig {
        sector_size: sector_size.into(),
        sector_count,
        challenge_count: WINDOW_POST_CHALLENGE_COUNT,
        typ: PoStType::Window,
        priority: false,
        api_version,
    };

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

   FallbackPoStCompound::


    let rt = Runtime::new().unwrap();

    let mut client = rt.block_on(async {
        match new_client("http://127.0.0.1:50051", Duration::from_secs(10)).await {
            Ok(c) => c,
            Err(e) => {
                panic!("{}", e)
            }
        }
    });

    let task_id = Uuid::new_v4();

    // lock server
    let req_lock_server = Request::new(GetWorkerStatusRequest { task_id: task_id.clone().to_string() });
    rt.block_on(async {
        match client.lock_server_if_free(req_lock_server).await {
            Ok(res) => {
                println!("{}", res.into_inner().msg)
            }
            Err(s) => {
                panic!("{}", s.message())
            }
        }
    });

    let randomness_safe = as_safe_commitment(&randomness, "randomness")?;
    let prover_id_safe = as_safe_commitment(&prover_id, "prover_id")?;
    let mut pub_sectors = Vec::with_capacity(vanilla_proofs.len());

    let pub_inputs = fallback::PublicInputs {
        randomness: randomness_safe,
        prover_id: prover_id_safe,
        sectors: pub_sectors,
        k: None,
    };

    // do task
    let req_do_task = Request::new(SnarkTaskRequestParams {
        task_id: task_id.clone().to_string(),
        vanilla_proof: serde_json::to_vec(&vanilla_proofs)?,
        pub_in: serde_json::to_vec(&pub_inputs)?,
        post_config: serde_json::to_vec(&config)?,
        replicas_len: priv_replicas.len() as u32,
    });

    rt.block_on(async {
        match client.do_snark_task(req_do_task).await {
            Ok(r) => {
                println!("{}", r.into_inner().msg)
            }
            Err(s) => {
                panic!("{}", s.message())
            }
        }
    });

    // get result
    let req_get_result = GetTaskResultRequest { task_id: task_id.clone().to_string() };
    rt.block_on(
        async {
            loop {
                match client.get_snark_task_result(Request::new(req_get_result.clone())).await {
                    Ok(res) => {
                        let r = res.into_inner();
                        if r.msg == "ok".to_string() {
                            println!("{:?}", r.result);
                            break;
                        } else {
                            tokio::time::sleep(Duration::from_secs(2)).await;
                            continue;
                        }
                    }
                    Err(s) => {
                        panic!("{}", s.message())
                    }
                }
            }
        }
    );

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

    do_window_post::<SectorShape2KiB>(
        sector_size,
        2 * sector_count,
        sector_count,
        false,
        ApiVersion::V1_0_0,
    )?;
    do_window_post::<SectorShape2KiB>(
        sector_size,
        2 * sector_count,
        sector_count,
        true,
        ApiVersion::V1_0_0,
    )?;
    do_window_post::<SectorShape2KiB>(
        sector_size,
        2 * sector_count,
        sector_count,
        false,
        ApiVersion::V1_1_0,
    )?;
    do_window_post::<SectorShape2KiB>(
        sector_size,
        2 * sector_count,
        sector_count,
        true,
        ApiVersion::V1_1_0,
    )
}

