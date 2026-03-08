use alloy::sol;

sol! {
    #[sol(rpc)]
    interface ILpManager {
        struct AllocateParams {
            address token;
            address usdc;
            uint256 tokenAmount;
            uint256 usdcAmount;
            uint256 tokenPrice;
        }

        // Events
        event LiquidityAllocated(address indexed token, address indexed pool, uint256 tokenAmount, int24 tickLower, int24 tickUpper);
        event FeesCollected(address indexed token, uint256 amount0, uint256 amount1);

        // Errors
        error OnlyIDO();
        error OnlyPoolManager();
        error ZeroAddress();
        error InvalidTokenAmount();
        error PositionNotFound();

        // Functions
        function allocate(AllocateParams calldata params) external;
        function collectFees(address token, address recipient) external;
    }
}
