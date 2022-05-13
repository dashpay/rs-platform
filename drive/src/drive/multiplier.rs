use crate::error::drive::DriveError::{MultiplierEncodingNotSupported, MultiplierNotSupported};
use crate::error::Error;

// The multiplier is encoded on a single byte, of which the first bit is reserved
pub struct Multiplier {
    byte: u8,
}

impl Multiplier {
    pub fn from_byte(byte: u8) -> Result<Self, Error> {
        if byte & 0x80 {
            Err(Error::Drive(MultiplierNotSupported(
                "Multipliers have the first bit reserved for future use",
            )))
        } else {
            Ok(Multiplier { byte })
        }
    }

    // We are trying to encode 7 bits to a float.
    //
    // The requirements are the following:
    // * We want more precision around a value of 1.
    // * The multiplier should be a rational number that is also a terminating decimal number.
    //   (i.e 0.25, not 0.3333333...)
    // * The original multiplier of 1 should equate to roughly at cost for a Dash price of 100 dollars.
    // * The multiplier should be easily understandable to follow the Dash price
    //
    // V1 (at release) will not let the masternode network change the multiplier.
    //
    // In v1 the base multiplier will be chosen to be roughly twice the cost to the network.
    // Example recommended values:
    // * 1 Dash = 100$, cost 1, multiplier 2
    // * 1 Dash = 200$, cost 0.5, multiplier 4
    // * 1 Dash = 50$, cost 2, multiplier 1
    // * 1 Dash = 10$, cost 10, multiplier 0.2
    // * 1 Dash = 500$, cost 0.2, multiplier 10
    // * 1 Dash = 2,000$, cost 0.05, multiplier 40
    // * 1 Dash = 5,000$, cost 0.02, multiplier 100
    // * 1 Dash = 10,000$, cost 0.01, multiplier 200
    // * 1 Dash = 20,000$, cost 0.005, multiplier 500
    // * 1 Dash = 40,000$, cost 0.0025, multiplier 1000
    // * 1 Dash = 200,000$, cost 0.0005, multiplier 4000
    // * 1 Dash = 1,000,000$, cost 0.0001, multiplier 20000
    //
    //
    // Solution :
    // * The lowest allowed multiplier would be 0.2, which would make sense at a Dash price of 10$.
    // * The highest allowed multiplier would be 20000, which would make sense at a Dash price of 1 Million $.
    // * Between [0.2 and 2] the minimal step is 0.2 (10$). This gives 10 possible values.
    // * Between ]2 and 10] the minimal step is 0.4 (20$). This gives us 20 possible values.
    // * Between ]10 and 40] the minimal step is 1 (50$). This gives us 30 possible values.
    // * Between ]40 and 200] the minimal step is 8 (400$). This gives us 20 possible values.
    // * Between ]200 and 1000] the minimal step is 40 (2,000$). This gives us 20 possible values.
    // * Between ]1000 and 4000] the minimal step is 200 (10,000$). This gives us 15 possible values.
    // * Between ]4000 and 10000] the minimal step is 1000 (50,000$). This gives us 6 possible values.
    // * Between ]10000 and 20000] the minimal step is 2000 (100,000$). This gives us 5 possible values.
    // This adds up to 126 possible values.

    // A byte value of 0111 1111 (127) is reserved.

    fn multiplier_value(&self) -> Result<Self, Error> {
        match self.byte {
            // * Between [0.2 and 2] the minimal step is 0.2 (10$). This gives 10 possible values.
            0..=9 => {
                0.2 + self.byte*0.2
            }
            // * Between ]2 and 10] the minimal step is 0.4 (20$). This gives us 20 possible values.
            10..=29 => {
                2 + (self.byte-10)*0.4
            }
            // * Between ]10 and 40] the minimal step is 1 (50$). This gives us 30 possible values.
            30..=59 => {
                10 + self.byte - 30
            }
            // * Between ]40 and 200] the minimal step is 8 (400$). This gives us 20 possible values.
            60..=79 => {
                40 + (self.byte-60)*8
            }
            // * Between ]200 and 1000] the minimal step is 40 (2,000$). This gives us 20 possible values.
            80..=99 => {
                200 + (self.byte-80)*40
            }
            // * Between ]1000 and 4000] the minimal step is 200 (10,000$). This gives us 15 possible values.
            100..=114 => {
                1000 + (self.byte-100)*200
            }
            // * Between ]4000 and 10000] the minimal step is 1000 (50,000$). This gives us 6 possible values.
            115..=120 => {
                4000 + (self.byte-115)*1000
            }
            // * Between ]10000 and 20000] the minimal step is 2000 (100,000$). This gives us 5 possible values.
            120..=124 => {
                10000 + (self.byte-120)*2000
            }
            _ => {
                Err(Error::Drive(MultiplierEncodingNotSupported(
                    "Value not supported",
                )))
            }
        }
    }

    pub fn multiply_fee(&self, fee: u64) -> u64 {}
}
