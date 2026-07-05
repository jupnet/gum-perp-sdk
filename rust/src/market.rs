use {
    crate::ValidationError,
    serde::{Deserialize, Serialize},
};

pub const QUOTE_DECIMALS: u8 = 6;

pub trait MarketParams {
    fn base_decimals(&self) -> u8;
    fn quote_lot_size(&self) -> u64;
    fn base_lot_size(&self) -> u64;

    fn tick_size(&self) -> f64 {
        self.quote_lot_size() as f64 * 10_f64.powi(self.base_decimals() as i32)
            / (10_f64.powi(QUOTE_DECIMALS as i32) * self.base_lot_size() as f64)
    }

    fn step_size(&self) -> f64 {
        self.base_lot_size() as f64 / 10_f64.powi(self.base_decimals() as i32)
    }

    fn to_base_lots(&self, quantity: f64) -> Result<u64, ValidationError> {
        let base_lots = (quantity
            * (10_u64.pow(self.base_decimals() as u32) / self.base_lot_size()) as f64)
            .round() as u64;
        if base_lots == 0 {
            return Err(ValidationError::InvalidInput(
                "Quantity too small for minimum lot size".to_string(),
            ));
        }
        Ok(base_lots)
    }

    fn to_lots(&self, price: f64, quantity: f64) -> Result<(u64, u64), ValidationError> {
        let base_lots = self.to_base_lots(quantity)?;
        let multiplier = 10_u64.pow(QUOTE_DECIMALS as u32) * self.base_lot_size();
        let divisor = 10_u64.pow(self.base_decimals() as u32) * self.quote_lot_size();
        let price_lots = (price * multiplier as f64).round() as u64 / divisor;
        if price_lots == 0 {
            return Err(ValidationError::InvalidInput(
                "Price too small for minimum tick size".to_string(),
            ));
        }
        Ok((price_lots, base_lots))
    }

    fn price_per_lot_to_decimal(&self, price_per_lot: u64) -> f64 {
        let numerator = price_per_lot as f64 * 10_f64.powi(self.base_decimals() as i32);
        let denominator = 10_f64.powi(QUOTE_DECIMALS as i32) * self.base_lot_size() as f64;
        numerator / denominator
    }

    fn base_lots_to_decimal(&self, base_lots: u64) -> f64 {
        (base_lots as f64 * self.base_lot_size() as f64) / 10_f64.powi(self.base_decimals() as i32)
    }

    fn avg_entry_price_to_decimals(&self, entry_cost_native: i64, base_position_lots: i64) -> f64 {
        if base_position_lots == 0 {
            return 0.0;
        }
        let avg_per_base_lot = entry_cost_native as f64 / base_position_lots.abs() as f64;
        let numerator = avg_per_base_lot * 10_f64.powi(self.base_decimals() as i32);
        let denominator = 10_f64.powi(QUOTE_DECIMALS as i32) * self.base_lot_size() as f64;
        numerator / denominator
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct MarketStats {
    pub open_interest: f64,
    pub open_interest_base: f64,
    pub high_24h: f64,
    pub low_24h: f64,
    pub volume_24h: f64,
    pub last_trade_price: f64,
    pub price_change_24h: f64,
    pub price_change_percent_24h: f64,
    pub stats_last_updated_ms: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct MarketInfo {
    pub symbol: String,
    pub pubkey: String,
    pub base_decimals: u8,
    pub base_lot_size: u64,
    pub quote_lot_size: u64,
    pub tick_size: f64,
    pub step_size: f64,
    pub index_price: f64,
    pub mark_price: f64,
    pub best_bid: f64,
    pub best_ask: f64,
    pub mid_price: f64,
    pub initial_margin_ratio_bps: u16,
    pub maintenance_margin_ratio_bps: u16,
    pub taker_fee_bps: f64,
    pub maker_fee_bps: f64,
    pub liquidation_fee_bps: f64,
    pub timestamp: u64,
    pub last_updated_ms: u64,
    #[serde(flatten)]
    pub stats: MarketStats,
}

impl MarketParams for MarketInfo {
    fn base_decimals(&self) -> u8 {
        self.base_decimals
    }
    fn quote_lot_size(&self) -> u64 {
        self.quote_lot_size
    }
    fn base_lot_size(&self) -> u64 {
        self.base_lot_size
    }
}
