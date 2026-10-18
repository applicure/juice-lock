/**
 * Program IDL in camelCase format in order to be used in JS/TS.
 *
 * Note that this is only a type helper and is not the actual IDL. The original
 * IDL can be found at `target/idl/juice_lock.json`.
 */
export type JuiceLock = {
  "address": "544vBp5c5GHJwNDm4YmS4FK24to9YP17iJLSXmcjFtmG",
  "metadata": {
    "name": "juiceLock",
    "version": "0.1.0",
    "spec": "0.1.0",
    "description": "Juice LP locks: PumpSwap LP tokens locked for a fixed time, withdrawable only by their owner after it"
  },
  "instructions": [
    {
      "name": "extend",
      "docs": [
        "The owner lengthens a lock: the new unlock time (`now + DURATIONS[new_tier]`,",
        "or Forever) must be later than the current one. Also after expiry."
      ],
      "discriminator": [
        228,
        127,
        0,
        1,
        227,
        154,
        54,
        168
      ],
      "accounts": [
        {
          "name": "owner",
          "signer": true,
          "relations": [
            "lockRecord"
          ]
        },
        {
          "name": "lockRecord",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  108,
                  111,
                  99,
                  107
                ]
              },
              {
                "kind": "account",
                "path": "lockRecord.owner",
                "account": "lockRecord"
              },
              {
                "kind": "account",
                "path": "lockRecord.lpMint",
                "account": "lockRecord"
              },
              {
                "kind": "account",
                "path": "lockRecord.nonce",
                "account": "lockRecord"
              }
            ]
          }
        }
      ],
      "args": [
        {
          "name": "newTier",
          "type": "u8"
        }
      ]
    },
    {
      "name": "initConfig",
      "docs": [
        "Once, before the revoke: the upgrade authority creates the config."
      ],
      "discriminator": [
        23,
        235,
        115,
        232,
        168,
        96,
        1,
        231
      ],
      "accounts": [
        {
          "name": "authority",
          "signer": true
        },
        {
          "name": "program",
          "address": "544vBp5c5GHJwNDm4YmS4FK24to9YP17iJLSXmcjFtmG"
        },
        {
          "name": "programData"
        },
        {
          "name": "payer",
          "writable": true,
          "signer": true
        },
        {
          "name": "config",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  108,
                  111,
                  99,
                  107,
                  95,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": [
        {
          "name": "admin",
          "type": "pubkey"
        },
        {
          "name": "treasury",
          "type": "pubkey"
        },
        {
          "name": "feeLamports",
          "type": "u64"
        }
      ]
    },
    {
      "name": "lock",
      "docs": [
        "The owner locks `amount` LP of a PumpSwap pool for `tier`",
        "(0..=4: 7, 30, 90, 180, 365 days; 5: Forever), paying the fee to the",
        "treasury. `nonce` (chosen by the client) tells the owner's locks of one",
        "mint apart. `max_fee`: the most the owner accepts to pay (the fee it",
        "was shown), so a fee raised before the transaction lands is refused."
      ],
      "discriminator": [
        21,
        19,
        208,
        43,
        237,
        62,
        255,
        87
      ],
      "accounts": [
        {
          "name": "owner",
          "docs": [
            "Pays the fee and both rents. Any signer, a PDA included."
          ],
          "writable": true,
          "signer": true
        },
        {
          "name": "config",
          "docs": [
            "Read only: `lock` writes nothing to the config."
          ],
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  108,
                  111,
                  99,
                  107,
                  95,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "treasury",
          "writable": true
        },
        {
          "name": "pumpPool",
          "docs": [
            "discriminator, LP mint PDA, mint authority)."
          ]
        },
        {
          "name": "lpMint",
          "docs": [
            "is the token program (constraint on `token_program`)."
          ]
        },
        {
          "name": "ownerToken",
          "writable": true
        },
        {
          "name": "lockRecord",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  108,
                  111,
                  99,
                  107
                ]
              },
              {
                "kind": "account",
                "path": "owner"
              },
              {
                "kind": "account",
                "path": "lpMint"
              },
              {
                "kind": "arg",
                "path": "nonce"
              }
            ]
          }
        },
        {
          "name": "vault",
          "docs": [
            "`lp_mint` under `token_program`, authority `lock_record`. Never an ATA."
          ],
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  118,
                  97,
                  117,
                  108,
                  116
                ]
              },
              {
                "kind": "account",
                "path": "lockRecord"
              }
            ]
          }
        },
        {
          "name": "tokenProgram"
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": [
        {
          "name": "tier",
          "type": "u8"
        },
        {
          "name": "amount",
          "type": "u64"
        },
        {
          "name": "nonce",
          "type": "u64"
        },
        {
          "name": "maxFee",
          "type": "u64"
        }
      ]
    },
    {
      "name": "setAdmin",
      "docs": [
        "Admin, with the new admin's signature."
      ],
      "discriminator": [
        251,
        163,
        0,
        52,
        91,
        194,
        187,
        92
      ],
      "accounts": [
        {
          "name": "admin",
          "signer": true
        },
        {
          "name": "config",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  108,
                  111,
                  99,
                  107,
                  95,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "newAdmin",
          "signer": true
        }
      ],
      "args": []
    },
    {
      "name": "setConfig",
      "docs": [
        "Admin: the treasury, the fee (0 or within `[rent minimum, MAX_FEE_LAMPORTS]`)",
        "and the pause of new locks. Touches nothing but the config."
      ],
      "discriminator": [
        108,
        158,
        154,
        175,
        212,
        98,
        52,
        66
      ],
      "accounts": [
        {
          "name": "admin",
          "signer": true
        },
        {
          "name": "config",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  108,
                  111,
                  99,
                  107,
                  95,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        }
      ],
      "args": [
        {
          "name": "treasury",
          "type": "pubkey"
        },
        {
          "name": "feeLamports",
          "type": "u64"
        },
        {
          "name": "pausedNew",
          "type": "bool"
        }
      ]
    },
    {
      "name": "unlock",
      "docs": [
        "The owner, from the unlock time on: the vault's whole balance to a",
        "token account the owner owns; vault and record closed, rent to the owner."
      ],
      "discriminator": [
        101,
        155,
        40,
        21,
        158,
        189,
        56,
        203
      ],
      "accounts": [
        {
          "name": "owner",
          "docs": [
            "Receives both rents."
          ],
          "writable": true,
          "signer": true,
          "relations": [
            "lockRecord"
          ]
        },
        {
          "name": "lockRecord",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  108,
                  111,
                  99,
                  107
                ]
              },
              {
                "kind": "account",
                "path": "lockRecord.owner",
                "account": "lockRecord"
              },
              {
                "kind": "account",
                "path": "lockRecord.lpMint",
                "account": "lockRecord"
              },
              {
                "kind": "account",
                "path": "lockRecord.nonce",
                "account": "lockRecord"
              }
            ]
          }
        },
        {
          "name": "vault",
          "writable": true,
          "relations": [
            "lockRecord"
          ]
        },
        {
          "name": "lpMint",
          "relations": [
            "lockRecord"
          ]
        },
        {
          "name": "destination",
          "docs": [
            "A token account of the lock's mint that the lock's owner owns."
          ],
          "writable": true
        },
        {
          "name": "tokenProgram"
        }
      ],
      "args": []
    }
  ],
  "accounts": [
    {
      "name": "lockConfig",
      "discriminator": [
        106,
        47,
        238,
        159,
        124,
        12,
        160,
        192
      ]
    },
    {
      "name": "lockRecord",
      "discriminator": [
        157,
        145,
        17,
        26,
        171,
        35,
        61,
        131
      ]
    }
  ],
  "events": [
    {
      "name": "adminChanged",
      "discriminator": [
        232,
        34,
        31,
        226,
        62,
        18,
        19,
        114
      ]
    },
    {
      "name": "configChanged",
      "discriminator": [
        147,
        25,
        86,
        98,
        98,
        77,
        78,
        192
      ]
    },
    {
      "name": "configInitialized",
      "discriminator": [
        181,
        49,
        200,
        156,
        19,
        167,
        178,
        91
      ]
    },
    {
      "name": "extended",
      "discriminator": [
        5,
        76,
        91,
        44,
        187,
        214,
        114,
        165
      ]
    },
    {
      "name": "locked",
      "discriminator": [
        188,
        53,
        118,
        62,
        64,
        12,
        198,
        84
      ]
    },
    {
      "name": "unlocked",
      "discriminator": [
        219,
        104,
        74,
        123,
        174,
        92,
        182,
        120
      ]
    }
  ],
  "errors": [
    {
      "code": 6000,
      "name": "notUpgradeAuthority",
      "msg": "Only the program's upgrade authority can do that"
    },
    {
      "code": 6001,
      "name": "notAdmin",
      "msg": "Only the lock admin can do that"
    },
    {
      "code": 6002,
      "name": "paused",
      "msg": "New locks are paused; extend and unlock always work"
    },
    {
      "code": 6003,
      "name": "badVersion",
      "msg": "Unsupported account layout version"
    },
    {
      "code": 6004,
      "name": "invalidConfigValue",
      "msg": "Config value outside its bounds"
    },
    {
      "code": 6005,
      "name": "mintNotAllowed",
      "msg": "The mint has a freeze authority or an extension this program refuses"
    },
    {
      "code": 6006,
      "name": "pumpPoolMismatch",
      "msg": "Not a PumpSwap pool, or the LP mint is not that pool's"
    },
    {
      "code": 6007,
      "name": "wrongTokenProgram",
      "msg": "Wrong token program for this mint"
    },
    {
      "code": 6008,
      "name": "wrongTokenOwner",
      "msg": "The token account belongs to someone else"
    },
    {
      "code": 6009,
      "name": "wrongMint",
      "msg": "Wrong mint for this lock"
    },
    {
      "code": 6010,
      "name": "wrongTreasury",
      "msg": "Not the treasury fixed in the config"
    },
    {
      "code": 6011,
      "name": "zeroAmount",
      "msg": "Amount must be positive"
    },
    {
      "code": 6012,
      "name": "invalidTier",
      "msg": "Tier out of range"
    },
    {
      "code": 6013,
      "name": "notOwner",
      "msg": "Only the lock's owner can do that"
    },
    {
      "code": 6014,
      "name": "stillLocked",
      "msg": "The lock has not reached its unlock time"
    },
    {
      "code": 6015,
      "name": "foreverLock",
      "msg": "A Forever lock can never be unlocked or changed"
    },
    {
      "code": 6016,
      "name": "notLater",
      "msg": "The new unlock time must be later than the current one"
    },
    {
      "code": 6017,
      "name": "wrongVault",
      "msg": "Wrong vault for this lock"
    },
    {
      "code": 6018,
      "name": "mathOverflow",
      "msg": "Arithmetic overflow"
    },
    {
      "code": 6019,
      "name": "feeTooHigh",
      "msg": "The lock fee is higher than the maximum the owner accepted"
    }
  ],
  "types": [
    {
      "name": "adminChanged",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "old",
            "type": "pubkey"
          },
          {
            "name": "new",
            "type": "pubkey"
          }
        ]
      }
    },
    {
      "name": "configChanged",
      "docs": [
        "`set_config`: the values before and after."
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "oldTreasury",
            "type": "pubkey"
          },
          {
            "name": "treasury",
            "type": "pubkey"
          },
          {
            "name": "oldFeeLamports",
            "type": "u64"
          },
          {
            "name": "feeLamports",
            "type": "u64"
          },
          {
            "name": "oldPausedNew",
            "type": "bool"
          },
          {
            "name": "pausedNew",
            "type": "bool"
          }
        ]
      }
    },
    {
      "name": "configInitialized",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "admin",
            "type": "pubkey"
          },
          {
            "name": "treasury",
            "type": "pubkey"
          },
          {
            "name": "feeLamports",
            "type": "u64"
          }
        ]
      }
    },
    {
      "name": "extended",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "lock",
            "type": "pubkey"
          },
          {
            "name": "oldTier",
            "type": "u8"
          },
          {
            "name": "newTier",
            "type": "u8"
          },
          {
            "name": "oldUnlockTs",
            "type": "i64"
          },
          {
            "name": "newUnlockTs",
            "type": "i64"
          }
        ]
      }
    },
    {
      "name": "lockConfig",
      "docs": [
        "Global settings, PDA [\"lock_config\"], created once by the upgrade",
        "authority (before the revoke). Neither `extend` nor `unlock` takes this",
        "account, so nothing here can stop them. Read-only in `lock`; there is no",
        "on-chain lock counter (the indexer counts locks off-chain)."
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "version",
            "type": "u8"
          },
          {
            "name": "bump",
            "type": "u8"
          },
          {
            "name": "pausedNew",
            "docs": [
              "Blocks `lock` only."
            ],
            "type": "bool"
          },
          {
            "name": "pad",
            "type": {
              "array": [
                "u8",
                5
              ]
            }
          },
          {
            "name": "admin",
            "docs": [
              "A stored key (the upgrade authority won't exist after the revoke)."
            ],
            "type": "pubkey"
          },
          {
            "name": "treasury",
            "docs": [
              "Receives the fee in lamports."
            ],
            "type": "pubkey"
          },
          {
            "name": "feeLamports",
            "docs": [
              "0, or within `[rent minimum of an empty account, MAX_FEE_LAMPORTS]`."
            ],
            "type": "u64"
          }
        ]
      }
    },
    {
      "name": "lockRecord",
      "docs": [
        "One lock, PDA [\"lock\", owner, lp_mint, nonce (u64 LE)], rent paid by the",
        "owner. Only `unlock` closes it, so a record that disappears was unlocked."
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "version",
            "type": "u8"
          },
          {
            "name": "bump",
            "type": "u8"
          },
          {
            "name": "vaultBump",
            "type": "u8"
          },
          {
            "name": "tier",
            "docs": [
              "0..=5: 1 w, 1 m, 3 m, 6 m, 1 y, Forever (of the last `lock` or `extend`)."
            ],
            "type": "u8"
          },
          {
            "name": "forever",
            "docs": [
              "`tier == FOREVER_TIER`; then `unlock_ts == i64::MAX`."
            ],
            "type": "bool"
          },
          {
            "name": "lpDecimals",
            "type": "u8"
          },
          {
            "name": "tokenProgram",
            "docs": [
              "`TOKEN_PROGRAM_SPL` or `TOKEN_PROGRAM_2022`."
            ],
            "type": "u8"
          },
          {
            "name": "pad",
            "type": "u8"
          },
          {
            "name": "owner",
            "docs": [
              "Any signer, a PDA included (multisig vaults)."
            ],
            "type": "pubkey"
          },
          {
            "name": "lpMint",
            "type": "pubkey"
          },
          {
            "name": "pumpPool",
            "docs": [
              "The PumpSwap pool the mint was proven against at `lock`."
            ],
            "type": "pubkey"
          },
          {
            "name": "vault",
            "docs": [
              "PDA [\"vault\", this record]: a token account of `lp_mint`, authority this record."
            ],
            "type": "pubkey"
          },
          {
            "name": "amount",
            "docs": [
              "What the vault received at `lock` (measured)."
            ],
            "type": "u64"
          },
          {
            "name": "startTs",
            "docs": [
              "`lock`'s clock time; never changes."
            ],
            "type": "i64"
          },
          {
            "name": "unlockTs",
            "docs": [
              "`start_ts + DURATIONS[tier]`, raised by `extend`; `i64::MAX` for Forever."
            ],
            "type": "i64"
          },
          {
            "name": "nonce",
            "type": "u64"
          },
          {
            "name": "extendedAt",
            "docs": [
              "0 until the first `extend`, then the time of the last one."
            ],
            "type": "i64"
          },
          {
            "name": "feePaid",
            "docs": [
              "Fee charged at `lock`."
            ],
            "type": "u64"
          }
        ]
      }
    },
    {
      "name": "locked",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "lock",
            "type": "pubkey"
          },
          {
            "name": "owner",
            "type": "pubkey"
          },
          {
            "name": "lpMint",
            "type": "pubkey"
          },
          {
            "name": "pumpPool",
            "type": "pubkey"
          },
          {
            "name": "amount",
            "docs": [
              "Measured: what the vault received."
            ],
            "type": "u64"
          },
          {
            "name": "tier",
            "type": "u8"
          },
          {
            "name": "startTs",
            "type": "i64"
          },
          {
            "name": "unlockTs",
            "docs": [
              "`i64::MAX` for Forever."
            ],
            "type": "i64"
          },
          {
            "name": "nonce",
            "type": "u64"
          },
          {
            "name": "fee",
            "docs": [
              "Lamports paid to the treasury."
            ],
            "type": "u64"
          }
        ]
      }
    },
    {
      "name": "unlocked",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "lock",
            "type": "pubkey"
          },
          {
            "name": "owner",
            "type": "pubkey"
          },
          {
            "name": "amount",
            "docs": [
              "The vault's whole balance, paid to `destination` (donations included)."
            ],
            "type": "u64"
          },
          {
            "name": "destination",
            "type": "pubkey"
          }
        ]
      }
    }
  ]
};
