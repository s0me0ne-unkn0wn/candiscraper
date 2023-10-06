use parity_scale_codec::{Decode, Encode};
use subxt::utils::H256;
use subxt::{events::Phase, OnlineClient, PolkadotConfig};

#[subxt::subxt(runtime_metadata_path = "polkadot.metadata")]
pub mod polkadot {}

#[tokio::main]
pub async fn main() {
    if let Err(err) = run().await {
        eprintln!("{err}");
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let api = OnlineClient::<PolkadotConfig>::from_url("wss://rpc.polkadot.io").await?;
    let mut stream = api.blocks().subscribe_finalized().await?;

    loop {
        let block = stream.next().await;
        let block = block.unwrap()?;
        println!("{}", block.number());
        let events = block.events().await?;
        for event in events.iter() {
            let event = event?;
            if matches!(event.phase(), Phase::ApplyExtrinsic(_))
                && event.pallet_name() == "ParaInclusion"
                && event.variant_name() == "CandidateIncluded"
            {
                let ev = event
                    .as_event::<polkadot::para_inclusion::events::CandidateIncluded>()?
                    .unwrap();
                let receipt =
                    ::polkadot_primitives::CandidateReceipt::<H256>::decode(&mut &*ev.0.encode())?;
                let candidate_hash = receipt.hash();
                let para_id = receipt.descriptor().para_id;
                println!("{:?} para {}", candidate_hash, para_id);
                if para_id == 2004.into() {
                    let candidate = format!("{candidate_hash:?}");
                    let prefix = &candidate[2..4];
                    let pov_url = format!("http://povs.today/polkadot/{prefix}/{candidate}");
                    let client = reqwest::Client::new();

                    let pov_req = client.get(&pov_url).send().await?;
                    let pov_bytes = pov_req.bytes().await?;

                    let pov: ::polkadot_node_primitives::AvailableData =
                        parity_scale_codec::decode_from_bytes(pov_bytes)?;

                    std::fs::write(
                        std::path::Path::new(&format!("{}.pov", candidate)),
                        pov.encode(),
                    )?;
                    std::fs::write(
                        std::path::Path::new(&format!("{}.rcp", candidate)),
                        receipt.encode(),
                    )?;

                    let vch = receipt.descriptor().validation_code_hash;
                    let vch_name = format!("{:?}.pvf", vch);
                    let vch_file = std::path::Path::new(&vch_name);
                    if !vch_file.exists() {
                        let vch_subxt = polkadot::runtime_types::polkadot_parachain::primitives::ValidationCodeHash::decode(&mut &*vch.encode())?;
                        let storage_query = polkadot::storage().paras().code_by_hash(vch_subxt);
                        let code = api
                            .storage()
                            .at(receipt.descriptor().relay_parent)
                            .fetch(&storage_query)
                            .await?
                            .unwrap();
                        std::fs::write(vch_file, code.encode())?;
                    }
                }
            }
        }
    }
}
