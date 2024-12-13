// Copyright (C) Parity Technologies (UK) Ltd.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
use crate::{CallFlags, Result, ReturnFlags, StorageFlags};
use paste::paste;

#[cfg(target_arch = "riscv64")]
mod riscv64;

macro_rules! hash_fn {
	( $name:ident, $bytes:literal ) => {
		paste! {
			#[doc = "Computes the " $name " " $bytes "-bit hash on the given input buffer."]
			#[doc = "\n# Notes\n"]
			#[doc = "- The `input` and `output` buffer may overlap."]
			#[doc = "- The output buffer is expected to hold at least " $bytes " bits."]
			#[doc = "- It is the callers responsibility to provide an output buffer that is large enough to hold the expected amount of bytes returned by the hash function."]
			#[doc = "\n# Parameters\n"]
			#[doc = "- `input`: The input data buffer."]
			#[doc = "- `output`: The output buffer to write the hash result to."]
			fn [<hash_ $name>](input: &[u8], output: &mut [u8; $bytes]);
		}
	};
}

/// Implements [`HostFn`] when compiled on supported architectures (RISC-V).
pub enum HostFnImpl {}

/// Defines all the host apis available to contracts.
pub trait HostFn: private::Sealed {
	/// Stores the address of the current contract into the supplied buffer.
	///
	/// # Parameters
	///
	/// - `output`: A reference to the output data buffer to write the address.
	fn address(output: &mut [u8; 20]);

	/// Lock a new delegate dependency to the contract.
	///
	/// Traps if the maximum number of delegate_dependencies is reached or if
	/// the delegate dependency already exists.
	///
	/// # Parameters
	///
	/// - `code_hash`: The code hash of the dependency. Should be decodable as an `T::Hash`. Traps
	///   otherwise.
	fn lock_delegate_dependency(code_hash: &[u8; 32]);

	/// Get the contract immutable data.
	///
	/// Traps if:
	/// - Called from within the deploy export.
	/// - Called by contracts that didn't set immutable data by calling `set_immutable_data` during
	///   their constructor execution.
	///
	/// # Parameters
	/// - `output`: A reference to the output buffer to write the immutable bytes.
	fn get_immutable_data(output: &mut &mut [u8]);

	/// Set the contract immutable data.
	///
	/// It is only valid to set non-empty immutable data in the constructor once.
	///
	/// Traps if:
	/// - Called from within the call export.
	/// - Called more than once.
	/// - The provided data was empty.
	///
	/// # Parameters
	/// - `data`: A reference to the data to be stored as immutable bytes.
	fn set_immutable_data(data: &[u8]);

	/// Stores the **reducible** balance of the current account into the supplied buffer.
	///
	/// # Parameters
	///
	/// - `output`: A reference to the output data buffer to write the balance.
	fn balance(output: &mut [u8; 32]);

	/// Stores the **reducible** balance of the supplied address into the supplied buffer.
	///
	/// # Parameters
	///
	/// - `addr`: The target address of which to retreive the free balance.
	/// - `output`: A reference to the output data buffer to write the balance.
	fn balance_of(addr: &[u8; 20], output: &mut [u8; 32]);

	/// Returns the [EIP-155](https://eips.ethereum.org/EIPS/eip-155) chain ID.
	fn chain_id(output: &mut [u8; 32]);

	/// Stores the call data size as little endian U256 value into the supplied buffer.
	///
	/// # Parameters
	///
	/// - `output`: A reference to the output data buffer to write the call data size.
	fn call_data_size(output: &mut [u8; 32]);

	/// Stores the current block number of the current contract into the supplied buffer.
	///
	/// # Parameters
	///
	/// - `output`: A reference to the output data buffer to write the block number.
	fn block_number(output: &mut [u8; 32]);

	/// Stores the block hash of the given block number into the supplied buffer.
	///
	/// # Parameters
	///
	/// - `block_number`: A reference to the block number buffer.
	/// - `output`: A reference to the output data buffer to write the block number.
	fn block_hash(block_number: &[u8; 32], output: &mut [u8; 32]);

	/// Call (possibly transferring some amount of funds) into the specified account.
	///
	/// # Parameters
	///
	/// - `flags`: See [`CallFlags`] for a documentation of the supported flags.
	/// - `callee`: The address of the callee. Should be decodable as an `T::AccountId`. Traps
	///   otherwise.
	/// - `ref_time_limit`: how much *ref_time* Weight to devote to the execution.
	/// - `proof_size_limit`: how much *proof_size* Weight to devote to the execution.
	/// - `deposit`: The storage deposit limit for instantiation. Passing `None` means setting no
	///   specific limit for the call, which implies storage usage up to the limit of the parent
	///   call.
	/// - `value`: The value to transfer into the contract.
	/// - `input`: The input data buffer used to call the contract.
	/// - `output`: A reference to the output data buffer to write the call output buffer. If `None`
	///   is provided then the output buffer is not copied.
	///
	/// # Errors
	///
	/// An error means that the call wasn't successful output buffer is returned unless
	/// stated otherwise.
	///
	/// - [CalleeReverted][`crate::ReturnErrorCode::CalleeReverted]: Output buffer is returned.
	/// - [CalleeTrapped][`crate::ReturnErrorCode::CalleeTrapped]
	/// - [TransferFailed][`crate::ReturnErrorCode::TransferFailed]
	/// - [OutOfResources][`crate::ReturnErrorCode::OutOfResources]
	fn call(
		flags: CallFlags,
		callee: &[u8; 20],
		ref_time_limit: u64,
		proof_size_limit: u64,
		deposit: Option<&[u8; 32]>,
		value: &[u8; 32],
		input_data: &[u8],
		output: Option<&mut &mut [u8]>,
	) -> Result;

	/// Call into the chain extension provided by the chain if any.
	///
	/// Handling of the input values is up to the specific chain extension and so is the
	/// return value. The extension can decide to use the inputs as primitive inputs or as
	/// in/out arguments by interpreting them as pointers. Any caller of this function
	/// must therefore coordinate with the chain that it targets.
	///
	/// # Note
	///
	/// If no chain extension exists the contract will trap with the `NoChainExtension`
	/// module error.
	///
	/// # Parameters
	///
	/// - `func_id`: The function id of the chain extension.
	/// - `input`: The input data buffer.
	/// - `output`: A reference to the output data buffer to write the call output buffer. If `None`
	///   is provided then the output buffer is not copied.
	///
	/// # Return
	///
	/// The chain extension returned value, if executed successfully.
	fn call_chain_extension(func_id: u32, input: &[u8], output: Option<&mut &mut [u8]>) -> u32;

	/// Call some dispatchable of the runtime.
	///
	/// # Parameters
	///
	/// - `call`: The call data.
	///
	/// # Return
	///
	/// Returns `Error::Success` when the dispatchable was successfully executed and
	/// returned `Ok`. When the dispatchable was executed but returned an error
	/// `Error::CallRuntimeFailed` is returned. The full error is not
	/// provided because it is not guaranteed to be stable.
	///
	/// # Comparison with `ChainExtension`
	///
	/// Just as a chain extension this API allows the runtime to extend the functionality
	/// of contracts. While making use of this function is generally easier it cannot be
	/// used in all cases. Consider writing a chain extension if you need to do perform
	/// one of the following tasks:
	///
	/// - Return data.
	/// - Provide functionality **exclusively** to contracts.
	/// - Provide custom weights.
	/// - Avoid the need to keep the `Call` data structure stable.
	fn call_runtime(call: &[u8]) -> Result;

	/// Stores the address of the caller into the supplied buffer.
	///
	/// If this is a top-level call (i.e. initiated by an extrinsic) the origin address of the
	/// extrinsic will be returned. Otherwise, if this call is initiated by another contract then
	/// the address of the contract will be returned.
	///
	/// If there is no address associated with the caller (e.g. because the caller is root) then
	/// it traps with `BadOrigin`.
	///
	/// # Parameters
	///
	/// - `output`: A reference to the output data buffer to write the caller address.
	fn caller(output: &mut [u8; 20]);

	/// Stores the origin address (initator of the call stack) into the supplied buffer.
	///
	/// If there is no address associated with the origin (e.g. because the origin is root) then
	/// it traps with `BadOrigin`. This can only happen through on-chain governance actions or
	/// customized runtimes.
	///
	/// # Parameters
	///
	/// - `output`: A reference to the output data buffer to write the origin's address.
	fn origin(output: &mut [u8; 20]);

	/// Checks whether the caller of the current contract is the origin of the whole call stack.
	///
	/// Prefer this over [`is_contract()`][`Self::is_contract`] when checking whether your contract
	/// is being called by a contract or a plain account. The reason is that it performs better
	/// since it does not need to do any storage lookups.
	///
	/// # Return
	///
	/// A return value of `true` indicates that this contract is being called by a plain account
	/// and `false` indicates that the caller is another contract.
	fn caller_is_origin() -> bool;

	/// Checks whether the caller of the current contract is root.
	///
	/// Note that only the origin of the call stack can be root. Hence this function returning
	/// `true` implies that the contract is being called by the origin.
	///
	/// A return value of `true` indicates that this contract is being called by a root origin,
	/// and `false` indicates that the caller is a signed origin.
	fn caller_is_root() -> u32;

	/// Clear the value at the given key in the contract storage.
	///
	/// # Parameters
	///
	/// - `key`: The storage key.
	///
	/// # Return
	///
	/// Returns the size of the pre-existing value at the specified key if any.
	fn clear_storage(flags: StorageFlags, key: &[u8]) -> Option<u32>;

	/// Retrieve the code hash for a specified contract address.
	///
	/// # Parameters
	///
	/// - `addr`: The address of the contract.
	/// - `output`: A reference to the output data buffer to write the code hash.
	///
	/// # Note
	///
	/// If `addr` is not a contract but the account exists then the hash of empty data
	/// `0xc5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470` is written,
	/// otherwise `zero`.
	fn code_hash(addr: &[u8; 20], output: &mut [u8; 32]);

	/// Retrieve the code size for a specified contract address.
	///
	/// # Parameters
	///
	/// - `addr`: The address of the contract.
	/// - `output`: A reference to the output data buffer to write the code size.
	///
	/// # Note
	///
	/// If `addr` is not a contract the `output` will be zero.
	fn code_size(addr: &[u8; 20], output: &mut [u8; 32]);

	/// Checks whether there is a value stored under the given key.
	///
	/// The key length must not exceed the maximum defined by the contracts module parameter.
	///
	/// # Parameters
	/// - `key`: The storage key.
	///
	/// # Return
	///
	/// Returns the size of the pre-existing value at the specified key if any.
	fn contains_storage(flags: StorageFlags, key: &[u8]) -> Option<u32>;

	/// Emit a custom debug message.
	///
	/// No newlines are added to the supplied message.
	/// Specifying invalid UTF-8 just drops the message with no trap.
	///
	/// This is a no-op if debug message recording is disabled which is always the case
	/// when the code is executing on-chain. The message is interpreted as UTF-8 and
	/// appended to the debug buffer which is then supplied to the calling RPC client.
	///
	/// # Note
	///
	/// Even though no action is taken when debug message recording is disabled there is still
	/// a non trivial overhead (and weight cost) associated with calling this function. Contract
	/// languages should remove calls to this function (either at runtime or compile time) when
	/// not being executed as an RPC. For example, they could allow users to disable logging
	/// through compile time flags (cargo features) for on-chain deployment. Additionally, the
	/// return value of this function can be cached in order to prevent further calls at runtime.
	fn debug_message(str: &[u8]) -> Result;

	/// Execute code in the context (storage, caller, value) of the current contract.
	///
	/// Reentrancy protection is always disabled since the callee is allowed
	/// to modify the callers storage. This makes going through a reentrancy attack
	/// unnecessary for the callee when it wants to exploit the caller.
	///
	/// # Parameters
	///
	/// - `flags`: See [`CallFlags`] for a documentation of the supported flags.
	/// - `address`: The address of the code to be executed. Should be decodable as an
	///   `T::AccountId`. Traps otherwise.
	/// - `ref_time_limit`: how much *ref_time* Weight to devote to the execution.
	/// - `proof_size_limit`: how much *proof_size* Weight to devote to the execution.
	/// - `deposit_limit`: The storage deposit limit for delegate call. Passing `None` means setting
	///   no specific limit for the call, which implies storage usage up to the limit of the parent
	///   call.
	/// - `input`: The input data buffer used to call the contract.
	/// - `output`: A reference to the output data buffer to write the call output buffer. If `None`
	///   is provided then the output buffer is not copied.
	///
	/// # Errors
	///
	/// An error means that the call wasn't successful and no output buffer is returned unless
	/// stated otherwise.
	///
	/// - [CalleeReverted][`crate::ReturnErrorCode::CalleeReverted]: Output buffer is returned.
	/// - [CalleeTrapped][`crate::ReturnErrorCode::CalleeTrapped]
	/// - [OutOfResources][`crate::ReturnErrorCode::OutOfResources]
	fn delegate_call(
		flags: CallFlags,
		address: &[u8; 20],
		ref_time_limit: u64,
		proof_size_limit: u64,
		deposit_limit: Option<&[u8; 32]>,
		input_data: &[u8],
		output: Option<&mut &mut [u8]>,
	) -> Result;

	/// Deposit a contract event with the data buffer and optional list of topics. There is a limit
	/// on the maximum number of topics specified by `event_topics`.
	///
	/// There should not be any duplicates in `topics`.
	///
	/// # Parameters
	///
	/// - `topics`: The topics list. It can't contain duplicates.
	fn deposit_event(topics: &[[u8; 32]], data: &[u8]);

	/// Recovers the ECDSA public key from the given message hash and signature.
	///
	/// Writes the public key into the given output buffer.
	/// Assumes the secp256k1 curve.
	///
	/// # Parameters
	///
	/// - `signature`: The signature bytes.
	/// - `message_hash`: The message hash bytes.
	/// - `output`: A reference to the output data buffer to write the public key.
	///
	/// # Errors
	///
	/// - [EcdsaRecoveryFailed][`crate::ReturnErrorCode::EcdsaRecoveryFailed]
	fn ecdsa_recover(
		signature: &[u8; 65],
		message_hash: &[u8; 32],
		output: &mut [u8; 33],
	) -> Result;

	/// Calculates Ethereum address from the ECDSA compressed public key and stores
	/// it into the supplied buffer.
	///
	/// # Parameters
	///
	/// - `pubkey`: The public key bytes.
	/// - `output`: A reference to the output data buffer to write the address.
	///
	/// # Errors
	///
	/// - [EcdsaRecoveryFailed][`crate::ReturnErrorCode::EcdsaRecoveryFailed]
	fn ecdsa_to_eth_address(pubkey: &[u8; 33], output: &mut [u8; 20]) -> Result;

	/// Stores the amount of weight left into the supplied buffer.
	/// The data is encoded as Weight.
	///
	/// If the available space in `output` is less than the size of the value a trap is triggered.
	///
	/// # Parameters
	///
	/// - `output`: A reference to the output data buffer to write the weight left.
	fn weight_left(output: &mut &mut [u8]);

	/// Retrieve the value under the given key from storage.
	///
	/// The key length must not exceed the maximum defined by the contracts module parameter.
	///
	/// # Parameters
	/// - `key`: The storage key.
	/// - `output`: A reference to the output data buffer to write the storage entry.
	///
	/// # Errors
	///
	/// [KeyNotFound][`crate::ReturnErrorCode::KeyNotFound]
	fn get_storage(flags: StorageFlags, key: &[u8], output: &mut &mut [u8]) -> Result;

	hash_fn!(sha2_256, 32);
	hash_fn!(keccak_256, 32);
	hash_fn!(blake2_256, 32);
	hash_fn!(blake2_128, 16);

	/// Stores the input passed by the caller into the supplied buffer.
	///
	/// # Note
	///
	/// This function traps if:
	/// - the input is larger than the available space.
	/// - the input was previously forwarded by a [`call()`][`Self::call()`].
	///
	/// # Parameters
	///
	/// - `output`: A reference to the output data buffer to write the input data.
	fn input(output: &mut &mut [u8]);

	/// Stores the U256 value at given `offset` from the input passed by the caller
	/// into the supplied buffer.
	///
	/// # Note
	/// - If `offset` is out of bounds, a value of zero will be returned.
	/// - If `offset` is in bounds but there is not enough call data, the available data
	/// is right-padded in order to fill a whole U256 value.
	/// - The data written to `output` is a little endian U256 integer value.
	///
	/// # Parameters
	///
	/// - `output`: A reference to the fixed output data buffer to write the value.
	/// - `offset`: The offset (index) into the call data.
	fn call_data_load(output: &mut [u8; 32], offset: u32);

	/// Instantiate a contract with the specified code hash.
	///
	/// This function creates an account and executes the constructor defined in the code specified
	/// by the code hash.
	///
	/// # Parameters
	///
	/// - `code_hash`: The hash of the code to be instantiated.
	/// - `ref_time_limit`: how much *ref_time* Weight to devote to the execution.
	/// - `proof_size_limit`: how much *proof_size* Weight to devote to the execution.
	/// - `deposit`: The storage deposit limit for instantiation. Passing `None` means setting no
	///   specific limit for the call, which implies storage usage up to the limit of the parent
	///   call.
	/// - `value`: The value to transfer into the contract.
	/// - `input`: The input data buffer.
	/// - `address`: A reference to the address buffer to write the address of the contract. If
	///   `None` is provided then the output buffer is not copied.
	/// - `output`: A reference to the return value buffer to write the constructor output buffer.
	///   If `None` is provided then the output buffer is not copied.
	/// - `salt`: The salt bytes to use for this instantiation.
	///
	/// # Errors
	///
	/// Please consult the [ReturnErrorCode][`crate::ReturnErrorCode`] enum declaration for more
	/// information on those errors. Here we only note things specific to this function.
	///
	/// An error means that the account wasn't created and no address or output buffer
	/// is returned unless stated otherwise.
	///
	/// - [CalleeReverted][`crate::ReturnErrorCode::CalleeReverted]: Output buffer is returned.
	/// - [CalleeTrapped][`crate::ReturnErrorCode::CalleeTrapped]
	/// - [TransferFailed][`crate::ReturnErrorCode::TransferFailed]
	/// - [OutOfResources][`crate::ReturnErrorCode::OutOfResources]
	fn instantiate(
		code_hash: &[u8; 32],
		ref_time_limit: u64,
		proof_size_limit: u64,
		deposit: Option<&[u8; 32]>,
		value: &[u8; 32],
		input: &[u8],
		address: Option<&mut [u8; 20]>,
		output: Option<&mut &mut [u8]>,
		salt: Option<&[u8; 32]>,
	) -> Result;

	/// Checks whether a specified address belongs to a contract.
	///
	/// # Parameters
	///
	/// - `address`: The address to check
	///
	/// # Return
	///
	/// Returns `true` if the address belongs to a contract.
	fn is_contract(address: &[u8; 20]) -> bool;

	/// Stores the minimum balance (a.k.a. existential deposit) into the supplied buffer.
	///
	/// # Parameters
	///
	/// - `output`: A reference to the output data buffer to write the minimum balance.
	fn minimum_balance(output: &mut [u8; 32]);

	/// Retrieve the code hash of the currently executing contract.
	///
	/// # Parameters
	///
	/// - `output`: A reference to the output data buffer to write the code hash.
	fn own_code_hash(output: &mut [u8; 32]);

	/// Load the latest block timestamp into the supplied buffer
	///
	/// # Parameters
	///
	/// - `output`: A reference to the output data buffer to write the timestamp.
	fn now(output: &mut [u8; 32]);

	/// Removes the delegate dependency from the contract.
	///
	/// Traps if the delegate dependency does not exist.
	///
	/// # Parameters
	///
	/// - `code_hash`: The code hash of the dependency. Should be decodable as an `T::Hash`. Traps
	///   otherwise.
	fn unlock_delegate_dependency(code_hash: &[u8; 32]);

	/// Cease contract execution and save a data buffer as a result of the execution.
	///
	/// This function never returns as it stops execution of the caller.
	/// This is the only way to return a data buffer to the caller. Returning from
	/// execution without calling this function is equivalent to calling:
	/// ```nocompile
	/// return_value(ReturnFlags::empty(), &[])
	/// ```
	///
	/// Using an unnamed non empty `ReturnFlags` triggers a trap.
	///
	/// # Parameters
	///
	/// - `flags`: Flag used to signal special return conditions to the supervisor. See
	///   [`ReturnFlags`] for a documentation of the supported flags.
	/// - `return_value`: The return value buffer.
	fn return_value(flags: ReturnFlags, return_value: &[u8]) -> !;

	/// Replace the contract code at the specified address with new code.
	///
	/// # Note
	///
	/// There are a couple of important considerations which must be taken into account when
	/// using this API:
	///
	/// 1. The storage at the code address will remain untouched. This means that contract
	/// developers must ensure that the storage layout of the new code is compatible with that of
	/// the old code.
	///
	/// 2. Contracts using this API can't be assumed as having deterministic addresses. Said another
	/// way, when using this API you lose the guarantee that an address always identifies a specific
	/// code hash.
	///
	/// 3. If a contract calls into itself after changing its code the new call would use
	/// the new code. However, if the original caller panics after returning from the sub call it
	/// would revert the changes made by [`set_code_hash()`][`Self::set_code_hash`] and the next
	/// caller would use the old code.
	///
	/// # Parameters
	///
	/// - `code_hash`: The hash of the new code. Should be decodable as an `T::Hash`. Traps
	///   otherwise.
	///
	/// # Panics
	///
	/// Panics if there is no code on-chain with the specified hash.
	fn set_code_hash(code_hash: &[u8; 32]);

	/// Set the value at the given key in the contract storage.
	///
	/// The key and value lengths must not exceed the maximums defined by the contracts module
	/// parameters.
	///
	/// # Parameters
	///
	/// - `key`: The storage key.
	/// - `encoded_value`: The storage value.
	///
	/// # Return
	///
	/// Returns the size of the pre-existing value at the specified key if any.
	fn set_storage(flags: StorageFlags, key: &[u8], value: &[u8]) -> Option<u32>;

	/// Verify a sr25519 signature
	///
	/// # Parameters
	///
	/// - `signature`: The signature bytes.
	/// - `message`: The message bytes.
	///
	/// # Errors
	///
	/// - [Sr25519VerifyFailed][`crate::ReturnErrorCode::Sr25519VerifyFailed]
	fn sr25519_verify(signature: &[u8; 64], message: &[u8], pub_key: &[u8; 32]) -> Result;

	/// Retrieve and remove the value under the given key from storage.
	///
	/// # Parameters
	/// - `key`: The storage key.
	/// - `output`: A reference to the output data buffer to write the storage entry.
	///
	/// # Errors
	///
	/// [KeyNotFound][`crate::ReturnErrorCode::KeyNotFound]
	fn take_storage(flags: StorageFlags, key: &[u8], output: &mut &mut [u8]) -> Result;

	/// Remove the calling account and transfer remaining **free** balance.
	///
	/// This function never returns. Either the termination was successful and the
	/// execution of the destroyed contract is halted. Or it failed during the termination
	/// which is considered fatal and results in a trap + rollback.
	///
	/// # Parameters
	///
	/// - `beneficiary`: The address of the beneficiary account
	///
	/// # Traps
	///
	/// - The contract is live i.e is already on the call stack.
	/// - Failed to send the balance to the beneficiary.
	/// - The deletion queue is full.
	fn terminate(beneficiary: &[u8; 20]) -> !;

	/// Stores the value transferred along with this call/instantiate into the supplied buffer.
	///
	/// # Parameters
	///
	/// - `output`: A reference to the output data buffer to write the transferred value.
	fn value_transferred(output: &mut [u8; 32]);

	/// Stores the price for the specified amount of gas into the supplied buffer.
	///
	/// # Parameters
	///
	/// - `ref_time_limit`: The *ref_time* Weight limit to query the price for.
	/// - `proof_size_limit`: The *proof_size* Weight limit to query the price for.
	/// - `output`: A reference to the output data buffer to write the price.
	fn weight_to_fee(ref_time_limit: u64, proof_size_limit: u64, output: &mut [u8; 32]);

	/// Execute an XCM program locally, using the contract's address as the origin.
	/// This is equivalent to dispatching `pallet_xcm::execute` through call_runtime, except that
	/// the function is called directly instead of being dispatched.
	///
	/// # Parameters
	///
	/// - `msg`: The message, should be decodable as a [VersionedXcm](https://paritytech.github.io/polkadot-sdk/master/staging_xcm/enum.VersionedXcm.html),
	///   traps otherwise.
	/// - `output`: A reference to the output data buffer to write the [Outcome](https://paritytech.github.io/polkadot-sdk/master/staging_xcm/v3/enum.Outcome.html)
	///
	/// # Return
	///
	/// Returns `Error::Success` when the XCM execution attempt is successful. When the XCM
	/// execution fails, `ReturnCode::XcmExecutionFailed` is returned
	fn xcm_execute(msg: &[u8]) -> Result;

	/// Send an XCM program from the contract to the specified destination.
	/// This is equivalent to dispatching `pallet_xcm::send` through `call_runtime`, except that
	/// the function is called directly instead of being dispatched.
	///
	/// # Parameters
	///
	/// - `dest`: The XCM destination, should be decodable as [VersionedLocation](https://paritytech.github.io/polkadot-sdk/master/staging_xcm/enum.VersionedLocation.html),
	///   traps otherwise.
	/// - `msg`: The message, should be decodable as a [VersionedXcm](https://paritytech.github.io/polkadot-sdk/master/staging_xcm/enum.VersionedXcm.html),
	///   traps otherwise.
	///
	/// # Return
	///
	/// Returns `ReturnCode::Success` when the message was successfully sent. When the XCM
	/// execution fails, `ReturnErrorCode::XcmSendFailed` is returned.
	fn xcm_send(dest: &[u8], msg: &[u8], output: &mut [u8; 32]) -> Result;

	/// Stores the size of the returned data of the last contract call or instantiation.
	///
	/// # Parameters
	///
	/// - `output`: A reference to the output buffer to write the size.
	fn return_data_size(output: &mut [u8; 32]);

	/// Stores the returned data of the last contract call or contract instantiation.
	///
	/// # Parameters
	/// - `output`: A reference to the output buffer to write the data.
	/// - `offset`: Byte offset into the returned data
	fn return_data_copy(output: &mut &mut [u8], offset: u32);
}

mod private {
	pub trait Sealed {}
	impl Sealed for super::HostFnImpl {}
}
