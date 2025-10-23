import { expect } from "chai";
import { network } from "hardhat";
import {
  ERC20TokenMock,
  ERC20TokenMock__factory,
  MockExchange,
  MockExchange__factory,
  SEOA,
  SEOA__factory,
} from "../typechain/index.js";
import { HardhatEthersSigner } from "@nomicfoundation/hardhat-ethers/types";
import {
  checkDelegationStatus,
  encodeExecuteBatch,
  encodeMode,
} from "../scripts/utils.js";

const { ethers } = await network.connect();
const EXECUTOR_ROLE = ethers.id("EXECUTOR_ROLE");

const delegationWizard = (sEOAImplementationAddress: string) => ({
  create: async (signer: HardhatEthersSigner, calldata?: string) => {
    const auth = await signer.authorize({
      address: sEOAImplementationAddress,
      nonce: (await signer.getNonce()) + 1,
    });

    const tx = await signer.sendTransaction({
      to: signer.address,
      authorizationList: [auth],
      data: calldata,
    });

    const receipt = await tx.wait();
    const sEOA = SEOA__factory.connect(signer.address, signer);

    return {
      receipt,
      sEOA,
    };
  },
  cancel: async (signer: HardhatEthersSigner) => {
    const auth = await signer.authorize({
      address: ethers.ZeroAddress,
      nonce: (await signer.getNonce()) + 1,
    });

    const tx = await signer.sendTransaction({
      to: signer.address,
      authorizationList: [auth],
    });

    return await tx.wait();
  },
});

describe("sEOA", function () {
  let sEOAImplementationAddress: string = "";
  let deployer: HardhatEthersSigner;
  let owner: HardhatEthersSigner;
  let external: HardhatEthersSigner;
  let delegation: ReturnType<typeof delegationWizard>;

  beforeEach(async () => {
    [deployer, owner, external] = await ethers.getSigners();

    if (!sEOAImplementationAddress) {
      const sEOAFactory = new SEOA__factory(deployer);
      const sEOAImplementation = await (
        await sEOAFactory.deploy()
      ).waitForDeployment();
      sEOAImplementationAddress = await sEOAImplementation.getAddress();
    }

    delegation = delegationWizard(sEOAImplementationAddress);
  });

  describe("Code delegation", () => {
    beforeEach(async () => {
      await delegation.cancel(owner);
    });

    it("works without calldata", async function () {
      expect(await checkDelegationStatus(owner)).to.be.false;

      const { receipt } = await delegation.create(owner);

      expect(receipt?.status).to.equal(1);

      expect(await checkDelegationStatus(owner)).to.be.true;
    });

    it("works with calldata and emits events", async function () {
      const calldata = SEOA__factory.createInterface().encodeFunctionData(
        "grantRole",
        [EXECUTOR_ROLE, external.address]
      );

      const { receipt, sEOA } = await delegation.create(owner, calldata);

      expect(receipt?.status).to.equal(1);

      const events = await sEOA.queryFilter(
        sEOA.getEvent("RoleGranted"),
        receipt?.blockNumber,
        receipt?.blockNumber
      );

      expect(events.length).to.equal(1);
      expect(events[0].args.role).to.equal(EXECUTOR_ROLE);
      expect(events[0].args.account.toLowerCase()).to.equal(
        external.address.toLowerCase()
      );

      expect(await sEOA.hasRole(EXECUTOR_ROLE, external.address)).to.equal(
        true
      );
    });

    it("allows sending txs from EOA as usual", async () => {
      await delegation.create(owner);

      const externalBalanceBefore = await external.provider?.getBalance(
        external.address
      );
      const ownerBalanceBefore = await owner.provider?.getBalance(
        owner.address
      );

      const res = await (
        await owner.sendTransaction({
          to: external.address,
          value: ethers.parseEther("1"),
        })
      ).wait();

      expect(res?.status).to.equal(1);
      expect(res?.logs.length).to.equal(0);

      const externalBalanceAfter = await external.provider?.getBalance(
        external.address
      );
      const ownerBalanceAfter = await owner.provider?.getBalance(owner.address);

      expect(externalBalanceAfter).to.equal(
        externalBalanceBefore! + ethers.parseEther("1")
      );
      expect(ownerBalanceAfter).to.equal(
        ownerBalanceBefore! -
          ethers.parseEther("1") -
          res?.gasUsed! * res?.gasPrice!
      );
    });

    it("allows interacting with sEOA", async () => {
      const [, , , , , , , differentSmartAccount] = await ethers.getSigners();
      const { sEOA } = await delegation.create(differentSmartAccount);

      // Self
      expect(await sEOA.hasRole(EXECUTOR_ROLE, external)).to.be.false;
      await (await sEOA.grantRole(EXECUTOR_ROLE, external)).wait();
      expect(await sEOA.hasRole(EXECUTOR_ROLE, external)).to.be.true;

      // Someone else - send 1 wei from sEOA balance, gas will be charged from external account balance
      const mode = encodeMode();
      const encodedBatch = encodeExecuteBatch([
        { target: external.address, value: 1n },
      ]);

      const externalBalanceBefore = await ethers.provider.getBalance(
        external.address
      );
      const ownerBalanceBefore = await ethers.provider.getBalance(
        differentSmartAccount.address
      );

      const res = await (
        await sEOA.connect(external).execute(mode, encodedBatch)
      ).wait();

      const externalBalanceAfter = await external.provider?.getBalance(
        external.address
      );
      const ownerBalanceAfter =
        await differentSmartAccount.provider?.getBalance(
          differentSmartAccount.address
        );

      expect(ownerBalanceAfter).to.equal(ownerBalanceBefore! - 1n);
      expect(externalBalanceAfter).to.equal(
        externalBalanceBefore! + 1n - res?.gasUsed! * res?.gasPrice!
      );
    });

    it("can be cancelled", async () => {
      let { receipt } = await delegation.create(owner);

      expect(receipt?.status).to.equal(1);

      expect(await checkDelegationStatus(owner)).to.be.true;

      receipt = await delegation.cancel(owner);

      expect(receipt?.status).to.equal(1);

      expect(await checkDelegationStatus(owner)).to.be.false;
    });
  });

  describe("Execute", () => {
    let sEOA: SEOA;
    let USDC: ERC20TokenMock;
    let mockExchange: MockExchange;

    before(async () => {
      sEOA = (await delegation.create(owner)).sEOA;
      await (await sEOA.grantRole(EXECUTOR_ROLE, external.address)).wait();

      USDC = await (
        await new ERC20TokenMock__factory(deployer).deploy()
      ).waitForDeployment();
      await (await USDC.mintTo(owner.address, ethers.parseEther("1"))).wait();

      mockExchange = await (
        await new MockExchange__factory(deployer).deploy(
          await USDC.getAddress()
        )
      ).waitForDeployment();
      await (await USDC.mintTo(mockExchange, ethers.parseEther("1"))).wait();
      await (
        await deployer.sendTransaction({
          to: await mockExchange.getAddress(),
          value: ethers.parseEther("1"),
        })
      ).wait();
    });

    it("works for EOA", async () => {
      const ownerBalanceBefore = await USDC.balanceOf(owner.address);
      const externalBalanceBefore = await USDC.balanceOf(external.address);
      const calldata =
        ERC20TokenMock__factory.createInterface().encodeFunctionData(
          "transfer",
          [external.address, ethers.parseEther("0.1")]
        );
      const mode = encodeMode();
      const encodedBatch = encodeExecuteBatch([
        { target: await USDC.getAddress(), data: calldata },
      ]);
      const tx = sEOA.connect(owner).execute(mode, encodedBatch);

      await expect(tx).not.to.revert(ethers);

      const ownerBalanceAfter = await USDC.balanceOf(owner.address);
      const externalBalanceAfter = await USDC.balanceOf(external.address);

      expect(ownerBalanceAfter).to.equal(
        ownerBalanceBefore - ethers.parseEther("0.1")
      );
      expect(externalBalanceAfter).to.equal(
        externalBalanceBefore + ethers.parseEther("0.1")
      );
    });

    it("works for whitelisted signers (they pay gas)", async () => {
      const ownerBalanceBefore = await USDC.balanceOf(owner.address);
      const externalBalanceBefore = await USDC.balanceOf(external.address);
      const calldata =
        ERC20TokenMock__factory.createInterface().encodeFunctionData(
          "transfer",
          [external.address, ethers.parseEther("0.1")]
        );
      const mode = encodeMode();
      const encodedBatch = encodeExecuteBatch([
        { target: await USDC.getAddress(), data: calldata },
      ]);
      const tx = sEOA.connect(external).execute(mode, encodedBatch);

      await expect(tx).not.to.revert(ethers);

      const ownerBalanceAfter = await USDC.balanceOf(owner.address);
      const externalBalanceAfter = await USDC.balanceOf(external.address);

      expect(ownerBalanceAfter).to.equal(
        ownerBalanceBefore - ethers.parseEther("0.1")
      );
      expect(externalBalanceAfter).to.equal(
        externalBalanceBefore + ethers.parseEther("0.1")
      );
    });

    it("works for multiple transactions if all succeeds", async () => {
      const ownerUSDCBalanceBefore = await USDC.balanceOf(owner.address);
      const ownerEthBalanceBefore = await owner.provider?.getBalance(
        owner.address
      )!;
      const exchangeUSDCBalanceBefore = await USDC.balanceOf(
        await mockExchange.getAddress()
      );
      const exchangeEthBalanceBefore = await owner.provider?.getBalance(
        await mockExchange.getAddress()
      )!;

      const calldataForApprove =
        ERC20TokenMock__factory.createInterface().encodeFunctionData(
          "approve",
          [await mockExchange.getAddress(), ethers.parseEther("0.1")]
        );
      const calldataForSwap =
        MockExchange__factory.createInterface().encodeFunctionData(
          "swapToEth",
          [ethers.parseEther("0.1")]
        );

      const mode = encodeMode();
      const encodedBatch = encodeExecuteBatch([
        { target: await USDC.getAddress(), data: calldataForApprove },
        { target: await mockExchange.getAddress(), data: calldataForSwap },
      ]);

      const tx = sEOA.connect(external).execute(mode, encodedBatch);

      await expect(tx).not.to.revert(ethers);

      const ownerUSDCBalanceAfter = await USDC.balanceOf(owner.address);
      const ownerEthBalanceAfter = await owner.provider?.getBalance(
        owner.address
      )!;
      const exchangeUSDCBalanceAfter = await USDC.balanceOf(
        await mockExchange.getAddress()
      );
      const exchangeEthBalanceAfter = await owner.provider?.getBalance(
        await mockExchange.getAddress()
      )!;

      expect(ownerUSDCBalanceAfter).to.equal(
        ownerUSDCBalanceBefore - ethers.parseEther("0.1")
      );
      expect(ownerEthBalanceAfter).to.equal(ownerEthBalanceBefore + 1n);
      expect(exchangeUSDCBalanceAfter).to.equal(
        exchangeUSDCBalanceBefore + ethers.parseEther("0.1")
      );
      expect(exchangeEthBalanceAfter).to.equal(exchangeEthBalanceBefore - 1n);
    });

    it("reverts if one of multiple transactions fails", async () => {
      const calldataForApprove =
        ERC20TokenMock__factory.createInterface().encodeFunctionData(
          "approve",
          [await mockExchange.getAddress(), ethers.parseEther("0.01")]
        );
      const calldataForSwap =
        MockExchange__factory.createInterface().encodeFunctionData(
          "swapToEth",
          [ethers.parseEther("0.1")]
        );

      const mode = encodeMode();
      const encodedBatch = encodeExecuteBatch([
        { target: await USDC.getAddress(), data: calldataForApprove },
        { target: await mockExchange.getAddress(), data: calldataForSwap },
      ]);

      const tx = sEOA.connect(external).execute(mode, encodedBatch);

      await expect(tx).to.revertedWithCustomError(
        USDC,
        "ERC20InsufficientAllowance"
      );
    });

    // it('works for EOA through 4337')
    // it('works for whitelisted signers through 4337')
    // it('works for try/catch execType', async () => {})
    // it('works for single callType', async () => {})
    // it('works for batch callType', async () => {})
  });
  describe("AccessControl", () => {
    it("Can grant and revoke role only by self-call", async () => {
      const randomAddress1 = ethers.Wallet.createRandom().address;
      const randomAddress2 = ethers.Wallet.createRandom().address;

      const sEOA = (await delegation.create(owner)).sEOA;
      await sEOA.grantRole(EXECUTOR_ROLE, external);

      await expect(
        sEOA.connect(owner).grantRole(EXECUTOR_ROLE, randomAddress1)
      ).not.revert(ethers);
      await expect(
        sEOA.connect(external).grantRole(EXECUTOR_ROLE, randomAddress1)
      ).revert(ethers);

      await expect(
        sEOA.connect(external).revokeRole(EXECUTOR_ROLE, randomAddress2)
      ).revert(ethers);
      await expect(
        sEOA.connect(owner).revokeRole(EXECUTOR_ROLE, randomAddress2)
      ).not.revert(ethers);
    });
  });
});
