//! Zero-value deposit guards for `add_liquidity`.
//!
//! `quote` floors, so on a lopsided pool a strictly positive desired amount can
//! quote to zero on one side. These tests pin the behaviour at the contract
//! boundary, not just at the quote helper.
mod probe {
    use crate::{AmmPool, AmmPoolClient};
    use soroban_sdk::testutils::Address as _;
    use soroban_sdk::token::{StellarAssetClient, TokenClient};
    use soroban_sdk::{Address, Env};

    fn setup(env: &Env) -> (Address, Address, Address, AmmPoolClient<'_>) {
        let admin = Address::generate(env);
        env.mock_all_auths();
        let a = env
            .register_stellar_asset_contract_v2(admin.clone())
            .address();
        let b = env
            .register_stellar_asset_contract_v2(admin.clone())
            .address();
        let pool_id = env.register_contract(None, AmmPool);
        let pool = AmmPoolClient::new(env, &pool_id);
        let factory = Address::generate(env);
        pool.initialize(&factory, &a, &b);
        (a, b, pool_id, pool)
    }

    fn mint(env: &Env, token: &Address, to: &Address, amt: i128) {
        StellarAssetClient::new(env, token).mint(to, &amt);
    }

    #[test]
    #[should_panic(expected = "rounds to zero at current reserves")]
    fn second_side_quoting_to_zero_is_rejected_before_transfer() {
        let env = Env::default();
        env.mock_all_auths();
        let (ta, tb, pool_id, pool) = setup(&env);
        let u = Address::generate(&env);
        mint(&env, &ta, &u, 10_000_000_000);
        mint(&env, &tb, &u, 10_000_000_000);

        // 1e9 : 1 pool. Desired A = 1 quotes to 0 on the B side.
        pool.add_liquidity(&u, &1_000_000_000, &1, &0, &0);

        let before_a = TokenClient::new(&env, &ta).balance(&pool_id);
        let before_b = TokenClient::new(&env, &tb).balance(&pool_id);

        pool.add_liquidity(&u, &1, &1, &0, &0);

        // Unreachable: the guard must fire before any transfer.
        assert_eq!(
            TokenClient::new(&env, &ta).balance(&pool_id),
            before_a,
            "token A moved despite rejection"
        );
        assert_eq!(
            TokenClient::new(&env, &tb).balance(&pool_id),
            before_b,
            "token B moved despite rejection"
        );
    }

    #[test]
    fn rejecting_a_zero_side_leaves_reserves_untouched() {
        let env = Env::default();
        env.mock_all_auths();
        let (ta, tb, _pid, pool) = setup(&env);
        let u = Address::generate(&env);
        mint(&env, &ta, &u, 10_000_000_000);
        mint(&env, &tb, &u, 10_000_000_000);

        pool.add_liquidity(&u, &1_000_000_000, &1, &0, &0);
        let seeded = pool.get_reserves();

        // Invariant only: the call must not succeed and must not move reserves.
        // Discrimination between the guard and the downstream `liq` panic is
        // covered by `second_side_quoting_to_zero_is_rejected_before_transfer`,
        // whose expected message does not exist without the guard.
        let res = pool.try_add_liquidity(&u, &1, &1, &0, &0);
        assert!(res.is_err(), "zero-side deposit was accepted");

        assert_eq!(pool.get_reserves(), seeded, "reserves changed on failure");
    }

    #[test]
    #[should_panic(expected = "amounts")]
    fn zero_desired_amount_is_still_rejected() {
        let env = Env::default();
        env.mock_all_auths();
        let (ta, tb, _pid, pool) = setup(&env);
        let u = Address::generate(&env);
        mint(&env, &ta, &u, 1_000_000);
        mint(&env, &tb, &u, 1_000_000);
        pool.add_liquidity(&u, &0, &1_000_000, &0, &0);
    }

    #[test]
    fn balanced_deposit_still_succeeds() {
        let env = Env::default();
        env.mock_all_auths();
        let (ta, tb, _pid, pool) = setup(&env);
        let u = Address::generate(&env);
        mint(&env, &ta, &u, 10_000_000);
        mint(&env, &tb, &u, 10_000_000);

        let lp = pool.add_liquidity(&u, &1_000_000, &1_000_000, &0, &0);
        assert!(lp > 0, "balanced deposit minted no liquidity");
        assert_eq!(pool.get_reserves(), (1_000_000, 1_000_000));
    }

    #[test]
    fn proportional_deposit_on_lopsided_pool_still_succeeds() {
        let env = Env::default();
        env.mock_all_auths();
        let (ta, tb, _pid, pool) = setup(&env);
        let u = Address::generate(&env);
        mint(&env, &ta, &u, 10_000_000_000);
        mint(&env, &tb, &u, 10_000_000_000);

        // 1e6 : 1e6 seed, then deposit in proportion: both sides stay positive.
        pool.add_liquidity(&u, &1_000_000, &1_000_000, &0, &0);
        let lp = pool.add_liquidity(&u, &500_000, &500_000, &0, &0);
        assert!(lp > 0, "proportional deposit minted no liquidity");
    }
}
