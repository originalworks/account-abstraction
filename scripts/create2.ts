import { concat, ContractFactory, getAddress, hexlify, keccak256, Signer, zeroPadValue } from "ethers"

export interface DeterministicDeploymentOptions<Factory extends ContractFactory> {
  constructorArgs?: Parameters<Factory['getDeployTransaction']>
  salt?: string
  contractName?: string
}
/**
 * This ensures that deployed byte code + salt deploys same byte code to the same addresses across different chains.
 * */

export class DeterministicDeployment {
  private static _logging = false
  private static _factoryAddress = '0x4e59b44847b379578588920ca78fbf26c0b4956c'
  private static _factoryDeployer = '0x3fab184622dc19b6109349b94811493bf2a45362' // factory deployer
  private static _factoryDeploymentTx =
    '0xf8a58085174876e800830186a08080b853604580600e600039806000f350fe7fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe03601600081602082378035828234f58015156039578182fd5b8082525050506014600cf31ba02222222222222222222222222222222222222222222222222222222222222222a02222222222222222222222222222222222222222222222222222222222222222'

  private static log(...args: unknown[]) {
    if (this._logging) {
      console.log(...args)
    }
  }

  private static async _deployProxy(signer: Signer) {
    const provider = signer.provider!
    const code = await provider.getCode(this._factoryAddress)
    if (code === '0x') {
      this.log('Deploying create2 proxy for the first time...')
      // Fund deployer
      await (
        await signer.sendTransaction({
          to: this._factoryDeployer,
          value: '10000000000000000',
        })
      ).wait()

      const deployTx = await provider.broadcastTransaction(
        this._factoryDeploymentTx,
      )
      await deployTx.wait()
    }
  }

  private static _getCreate2Address(salt: string, bytecode: string): string {
    const create2Hash = keccak256(
      concat([
        "0xff",
        this._factoryAddress,
        salt,
        keccak256(bytecode),
      ].map((x) => (typeof x === "string" ? x : hexlify(x))))
    );

    return getAddress("0x" + create2Hash.slice(-40));
  }

  static async deploy<Factory extends ContractFactory>(
    factory: Factory,
    options?: DeterministicDeploymentOptions<Factory>,
  ) {
    const signer = factory.runner as Signer
    const constructorArgs = options?.constructorArgs?.length
      ? options.constructorArgs
      : ([] as any)

    const salt = options?.salt
      ? hexlify(zeroPadValue(options.salt, 32))
      : "0x" + "00".repeat(32);

    await this._deployProxy(signer)

    const deploymentUnsignedTx = await factory.getDeployTransaction(
      ...constructorArgs,
    )

    const bytecode = deploymentUnsignedTx.data!
    const predictedAddress = this._getCreate2Address(
      salt,
      bytecode,
    )
    
    const deployedCode = await signer.provider!.getCode(predictedAddress)

    if (deployedCode === '0x') {
      if (options?.contractName) {
        this.log(
          `Deploying ${options?.contractName} for the first time at ${predictedAddress}`,
        )
      }

      const deploymentTx = await signer.sendTransaction({to: this._factoryAddress, data: concat([salt, bytecode])})
      await deploymentTx.wait()
    }

    const contract = factory.attach(predictedAddress) as Awaited<
      ReturnType<Factory['deploy']>
    >

    return contract
  }
}