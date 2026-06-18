//! Minimal mock-transport example for the JSON-RPC executor boundary.
//!
//! Run with:
//!   cargo run --example rpc_executor -p tangent-sdk

use std::collections::VecDeque;

use serde::de::DeserializeOwned;
use serde_json::json;
use tangent_sdk::{
    CollateralDepositPlan, DeploymentManifest, JsonRpcRequest, JsonRpcResponse, JsonRpcRetryPolicy,
    JsonRpcTransport, RawTransactionSigner, RawTransactionSigningRequest, RetryingJsonRpcTransport,
    RpcBlockTag, RpcEndpointConfig, SignedRawTransaction, TangentClient, TangentClientPlan,
};

#[derive(Debug, Default)]
struct MockTransport {
    responses: VecDeque<serde_json::Value>,
    methods: Vec<String>,
}

impl MockTransport {
    fn new(responses: impl IntoIterator<Item = serde_json::Value>) -> Self {
        Self {
            responses: responses.into_iter().collect(),
            methods: Vec::new(),
        }
    }
}

impl JsonRpcTransport for MockTransport {
    type Error = String;

    fn send<T: DeserializeOwned + Default>(
        &mut self,
        request: &JsonRpcRequest,
    ) -> Result<JsonRpcResponse<T>, Self::Error> {
        self.methods.push(request.method.clone());
        let response = self
            .responses
            .pop_front()
            .ok_or_else(|| format!("missing response for {}", request.method))?;
        serde_json::from_value(response).map_err(|error| error.to_string())
    }
}

#[derive(Debug, Default)]
struct MockRawSigner {
    requests: Vec<RawTransactionSigningRequest>,
}

impl RawTransactionSigner for MockRawSigner {
    type Error = String;

    fn sign_transaction(
        &mut self,
        request: &RawTransactionSigningRequest,
    ) -> Result<SignedRawTransaction, Self::Error> {
        self.requests.push(request.clone());
        SignedRawTransaction::from_hex("0x02abcd").map_err(|error| error.to_string())
    }
}

fn main() {
    let manifest =
        DeploymentManifest::from_json(include_str!("../../../docs/deployments/arc-testnet.json"))
            .expect("valid deployment manifest");
    let deposit_plan = CollateralDepositPlan::from_manifest(&manifest, 1, 1_000_000);
    let hash = format!("0x{}", "42".repeat(32));
    let transport = MockTransport::new([
        json!({"jsonrpc":"2.0","id":1,"result":"0x2b67"}),
        json!({"jsonrpc":"2.0","id":2,"result":"0x7"}),
        json!({"jsonrpc":"2.0","id":3,"result":"0x3d090"}),
        json!({"jsonrpc":"2.0","id":4,"result":"0x77359400"}),
        json!({"jsonrpc":"2.0","id":5,"result":"0x3b9aca00"}),
        json!({"jsonrpc":"2.0","id":6,"result": hash}),
    ]);
    let transport = RetryingJsonRpcTransport::new(transport, JsonRpcRetryPolicy::new(2));
    let client_plan = TangentClientPlan::new(
        manifest.clone(),
        RpcEndpointConfig::new("https://rpc.arc.example").expect("sample endpoint"),
    )
    .expect("client plan");
    let client = TangentClient::new(client_plan, transport);
    let mut workflow = client.into_workflow(MockRawSigner::default());

    let submission = workflow
        .preflight_sign_and_submit(
            &deposit_plan.approve_tx(),
            manifest.deployer,
            RpcBlockTag::Pending,
        )
        .expect("mock preflight, sign, and raw send");

    let (_, workflow) = workflow.into_parts();
    let (executor, signer) = workflow.into_parts();
    let transport = executor.into_transport();
    println!(
        "mock rpc methods : {}",
        transport.inner().methods.join(", ")
    );
    println!(
        "signed nonce     : {:?}",
        signer.requests[0].transaction.nonce
    );
    println!(
        "submitted hash   : {}",
        submission.transaction_hash.to_hex()
    );
}
