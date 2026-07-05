use {
    crate::{types::OnchainTimeInForce, Hash, OrderType, Pubkey, Side},
    borsh::{BorshDeserialize, BorshSerialize},
    std::num::NonZeroU64,
};

pub const JTX_PROGRAM_ID: &str = "jtxQHH3ysuWRETjaynai3jowPztq2LBJApVFcEv4Wmg";

const JTX_PROGRAM_ID_BYTES: [u8; 32] = [
    0x0a, 0xfd, 0x23, 0xd1, 0x13, 0x4d, 0x8b, 0x39, 0x51, 0x3c, 0x49, 0xc6, 0x77, 0x20, 0xab, 0xbf,
    0xfa, 0x8a, 0xdd, 0xf5, 0x0e, 0xb7, 0x83, 0xa8, 0x90, 0xba, 0xb5, 0xfb, 0x64, 0x73, 0xbe, 0xdb,
];

#[derive(BorshDeserialize, BorshSerialize, Default)]
pub struct PlaceOrderParams {
    pub subaccount_id: u8,
    pub symbol: String,
    pub price_lots: u64,
    pub base_lots: u64,
    pub side: Side,
    pub order_type: OrderType,
    pub reduce_only: bool,
    pub time_in_force: OnchainTimeInForce,
    pub client_order_id: Option<NonZeroU64>,
}

#[derive(BorshDeserialize, BorshSerialize, Default)]
pub struct PlaceOrderIsolatedParams {
    pub collateral_amount: u64,
    pub order: PlaceOrderParams,
}

#[derive(BorshDeserialize, BorshSerialize, Default)]
pub struct CancelOrderParams {
    pub subaccount_id: u8,
    pub symbol: String,
    pub order_id: Option<u128>,
    pub client_order_id: Option<NonZeroU64>,
}

#[derive(BorshDeserialize, BorshSerialize, Default)]
pub struct CancelAllOrdersParams {
    pub subaccount_id: u8,
    pub symbol: String,
}

pub enum TradingParams {
    Place(PlaceOrderParams),
    PlaceIsolated(PlaceOrderIsolatedParams),
    Cancel(CancelOrderParams),
    CancelIsolated(CancelOrderParams),
    CancelAll(CancelAllOrdersParams),
    CancelAllIsolated(CancelAllOrdersParams),
}

impl TradingParams {
    pub fn instruction_tag(&self) -> u8 {
        match self {
            Self::Place(_) => 3,
            Self::Cancel(_) => 4,
            Self::CancelAll(_) => 5,
            Self::PlaceIsolated(_) => 30,
            Self::CancelIsolated(_) => 31,
            Self::CancelAllIsolated(_) => 32,
        }
    }

    pub fn data(&self) -> Vec<u8> {
        let mut data = vec![self.instruction_tag()];
        match self {
            Self::Place(params) => borsh::to_writer(&mut data, params),
            Self::PlaceIsolated(params) => borsh::to_writer(&mut data, params),
            Self::Cancel(params) => borsh::to_writer(&mut data, params),
            Self::CancelIsolated(params) => borsh::to_writer(&mut data, params),
            Self::CancelAll(params) => borsh::to_writer(&mut data, params),
            Self::CancelAllIsolated(params) => borsh::to_writer(&mut data, params),
        }
        .expect("borsh serialization to Vec cannot fail");
        data
    }
}

pub fn serialize_action_message(signer: Pubkey, recent_blockhash: Hash, data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(32 + 32 + 1 + 32 + 3 + data.len());
    out.extend_from_slice(recent_blockhash.as_bytes());
    out.extend_from_slice(&JTX_PROGRAM_ID_BYTES);
    encode_short_vec_len(1, &mut out);
    out.extend_from_slice(signer.as_bytes());
    encode_short_vec_len(data.len(), &mut out);
    out.extend_from_slice(data);
    out
}

fn encode_short_vec_len(mut len: usize, out: &mut Vec<u8>) {
    loop {
        let mut elem = (len & 0x7f) as u8;
        len >>= 7;
        if len == 0 {
            out.push(elem);
            break;
        }
        elem |= 0x80;
        out.push(elem);
    }
}
