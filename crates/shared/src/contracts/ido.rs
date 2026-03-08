use alloy::sol;

sol! {
    #[sol(rpc)]
    interface IIDO {
        enum Status { Active, Graduated, Failed }

        struct Project {
            address creator;
            Status status;
            uint256 tokenPrice;
            uint256 idoSupply;
            uint256 idoSold;
            uint256 deadline;
            uint256 usdcRaised;
            uint256 usdcReleased;
            uint256 tokensRefunded;
        }

        struct Milestone {
            uint256 percentage;
            bool isApproved;
        }

        struct CreateParams {
            string name;
            string symbol;
            string tokenURI;
            uint256 idoTokenAmount;
            uint256 tokenPrice;
            uint256 deadline;
            uint256[] milestonePercentages;
            bytes32 salt;
        }

        // Events
        event ProjectCreated(
            address indexed token,
            address indexed creator,
            string name,
            string symbol,
            string tokenURI,
            uint256 idoTokenAmount,
            uint256 tokenPrice,
            uint256 deadline
        );
        event TokensPurchased(address indexed token, address indexed buyer, uint256 usdcAmount, uint256 tokenAmount);
        event Graduated(address indexed token);
        event MilestoneApproved(address indexed token, uint256 indexed milestoneIndex, uint256 usdcReleased);
        event ProjectFailed(address indexed token);
        event Refunded(address indexed token, address indexed buyer, uint256 tokensBurned, uint256 usdcReturned);
        event FeeManagerUpdated(address indexed newFeeManager);
        event LpManagerUpdated(address indexed newLpManager);
        event ProtocolTreasuryUpdated(address indexed newProtocolTreasury);

        // Errors
        error InvalidName();
        error InvalidSymbol();
        error InvalidTokenPrice();
        error InvalidDeadline();
        error InvalidMilestonePercentages();
        error InvalidIdoTokenAmount();
        error IDONotActive();
        error IDOExceedsSupply();
        error AlreadyGraduated();
        error IDONotFinished();
        error MilestoneAlreadyApproved();
        error InvalidMilestoneIndex();
        error ProjectAlreadyFailed();
        error ProjectNotFailed();
        error ZeroAmount();
        error ProjectNotFound();
        error CreatorCannotRefund();
        error InsufficientPurchaseAmount();
        error ZeroAddress();
        error TooManyMilestones();
        error ExceedsRefundableAmount();

        // State variables
        function USDC() external view returns (address);
        function TOKEN_IMPLEMENTATION() external view returns (address);
        function feeManager() external view returns (address);
        function lpManager() external view returns (address);
        function protocolTreasury() external view returns (address);
        function TOTAL_SUPPLY() external view returns (uint256);
        function MAX_MILESTONES() external view returns (uint256);
        function projects(address token) external view returns (Project memory);

        // Functions
        function create(CreateParams calldata params) external returns (address token);
        function buy(address token, uint256 usdcAmount) external;
        function graduate(address token) external;
        function approveMilestone(address token, uint256 milestoneIndex) external;
        function failProject(address token) external;
        function refund(address token, uint256 tokenAmount) external;
        function collectFees(address token) external;
        function getMilestones(address token) external view returns (Milestone[] memory);
    }
}
