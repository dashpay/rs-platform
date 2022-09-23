mod converter;

use neon::handle::Managed;
use std::ops::Deref;
use std::{option::Option::None, path::Path, sync::mpsc, thread};

use dash_abci::abci::handlers::TenderdashAbci;
use dash_abci::abci::messages::{
    BlockBeginRequest, BlockEndRequest, InitChainRequest, Serializable,
};
use dash_abci::platform::Platform;
use neon::prelude::*;
use neon::result::Throw;
use neon::types::JsDate;
use rs_drive::dpp::identity::Identity;
use rs_drive::drive::flags::StorageFlags;
use rs_drive::grovedb::{PathQuery, Transaction};

const READONLY_MSG: &str =
    "db is in readonly mode due to the active transaction. Please provide transaction or commit it";

type PlatformCallback = Box<dyn for<'a> FnOnce(&'a Platform, &Channel) + Send>;
type UnitCallback = Box<dyn FnOnce(&Channel) + Send>;

struct PlatformWrapperTransaction<'db>(Transaction<'db>);

impl<'db> PlatformWrapperTransaction<'db> {
    pub fn unwrap(self) -> Transaction<'db> {
        self.0
    }
}

unsafe impl<'db> Sync for PlatformWrapperTransaction<'db> {}
unsafe impl<'db> Send for PlatformWrapperTransaction<'db> {}

impl<'db> Deref for PlatformWrapperTransaction<'db> {
    type Target = Transaction<'db>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'db> Finalize for PlatformWrapperTransaction<'db> {}

// Messages sent on the drive channel
enum PlatformWrapperMessage {
    // Callback to be executed
    Callback(PlatformCallback),
    // Indicates that the thread should be stopped and connection closed
    Close(UnitCallback),
    // StartTransaction(UnitCallback),
    // CommitTransaction(UnitCallback),
    // RollbackTransaction(UnitCallback),
    // AbortTransaction(UnitCallback),
    Flush(UnitCallback),
}

struct PlatformWrapper {
    tx: mpsc::Sender<PlatformWrapperMessage>,
}

// Internal wrapper logic. Needed to avoid issues with passing threads to
// node.js. Avoiding thread conflicts by having a dedicated thread for the
// groveDB instance and uses events to communicate with it
impl PlatformWrapper {
    // Creates a new instance of `DriveWrapper`
    //
    // 1. Creates a connection and a channel
    // 2. Spawns a thread and moves the channel receiver and connection to it
    // 3. On a separate thread, read closures off the channel and execute with
    // access    to the connection.
    fn new(cx: &mut FunctionContext) -> NeonResult<Self> {
        let path_string = cx.argument::<JsString>(0)?.value(cx);

        // Channel for sending callbacks to execute on the Drive connection thread
        let (tx, rx) = mpsc::channel::<PlatformWrapperMessage>();

        // Create an `Channel` for calling back to JavaScript. It is more efficient
        // to create a single channel and re-use it for all database callbacks.
        // The JavaScript process will not exit as long as this channel has not been
        // dropped.
        let channel = cx.channel();

        // Spawn a thread for processing database queries
        // This will not block the JavaScript main thread and will continue executing
        // concurrently.
        thread::spawn(move || {
            let path = Path::new(&path_string);
            // Open a connection to groveDb, this will be moved to a separate thread
            // TODO: think how to pass this error to JS
            let platform: Platform = Platform::open(path, None).unwrap();

            // Blocks until a callback is available
            // When the instance of `Database` is dropped, the channel will be closed
            // and `rx.recv()` will return an `Err`, ending the loop and terminating
            // the thread.
            while let Ok(message) = rx.recv() {
                match message {
                    PlatformWrapperMessage::Callback(callback) => {
                        // The connection and channel are owned by the thread, but _lent_ to
                        // the callback. The callback has exclusive access to the connection
                        // for the duration of the callback.
                        callback(&platform, &channel);
                    }
                    // Immediately close the connection, even if there are pending messages
                    PlatformWrapperMessage::Close(callback) => {
                        drop(platform);
                        callback(&channel);
                        break;
                    }
                    // Flush message
                    PlatformWrapperMessage::Flush(callback) => {
                        platform.drive.grove.flush().unwrap();
                        callback(&channel);
                    } // PlatformWrapperMessage::StartTransaction(callback) => {
                      //     transaction = Some(platform.drive.grove.start_transaction());
                      //     callback(&channel);
                      // }
                      // PlatformWrapperMessage::CommitTransaction(callback) => {
                      //     platform
                      //         .drive
                      //         .commit_transaction(transaction.take().unwrap())
                      //         .unwrap();
                      //     callback(&channel);
                      // }
                      // PlatformWrapperMessage::RollbackTransaction(callback) => {
                      //     platform
                      //         .drive
                      //         .rollback_transaction(&transaction.take().unwrap())
                      //         .unwrap();
                      //     callback(&channel);
                      // }
                      // PlatformWrapperMessage::AbortTransaction(callback) => {
                      //     drop(transaction.take());
                      //     callback(&channel);
                      // }
                }
            }
        });

        Ok(Self { tx })
    }

    // Idiomatic rust would take an owned `self` to prevent use after close
    // However, it's not possible to prevent JavaScript from continuing to hold a
    // closed database
    fn close(
        &self,
        callback: impl FnOnce(&Channel) + Send + 'static,
    ) -> Result<(), mpsc::SendError<PlatformWrapperMessage>> {
        self.tx
            .send(PlatformWrapperMessage::Close(Box::new(callback)))
    }

    fn send_to_drive_thread(
        &self,
        callback: impl for<'a> FnOnce(&'a Platform, &Channel) + Send + 'static,
    ) -> Result<(), mpsc::SendError<PlatformWrapperMessage>> {
        self.tx
            .send(PlatformWrapperMessage::Callback(Box::new(callback)))
    }

    // fn start_transaction(
    //     &self,
    //     callback: impl FnOnce(&Channel) + Send + 'static,
    // ) -> Result<(), mpsc::SendError<PlatformWrapperMessage>> {
    //     self.tx
    //         .send(PlatformWrapperMessage::StartTransaction(Box::new(callback)))
    // }
    //
    // fn commit_transaction(
    //     &self,
    //     callback: impl FnOnce(&Channel) + Send + 'static,
    // ) -> Result<(), mpsc::SendError<PlatformWrapperMessage>> {
    //     self.tx
    //         .send(PlatformWrapperMessage::CommitTransaction(Box::new(callback)))
    // }
    //
    // fn rollback_transaction(
    //     &self,
    //     callback: impl FnOnce(&Channel) + Send + 'static,
    // ) -> Result<(), mpsc::SendError<PlatformWrapperMessage>> {
    //     self.tx
    //         .send(PlatformWrapperMessage::RollbackTransaction(Box::new(callback)))
    // }

    // Idiomatic rust would take an owned `self` to prevent use after close
    // However, it's not possible to prevent JavaScript from continuing to hold a
    // closed database
    fn flush(
        &self,
        callback: impl FnOnce(&Channel) + Send + 'static,
    ) -> Result<(), mpsc::SendError<PlatformWrapperMessage>> {
        self.tx
            .send(PlatformWrapperMessage::Flush(Box::new(callback)))
    }

    // fn abort_transaction(
    //     &self,
    //     callback: impl FnOnce(&Channel) + Send + 'static,
    // ) -> Result<(), mpsc::SendError<PlatformWrapperMessage>> {
    //     self.tx
    //         .send(PlatformWrapperMessage::AbortTransaction(Box::new(callback)))
    // }
}

// Ensures that DriveWrapper is properly disposed when the corresponding JS
// object gets garbage collected
impl Finalize for PlatformWrapper {}

// External wrapper logic
impl PlatformWrapper {
    // Create a new instance of `Drive` and place it inside a `JsBox`
    // JavaScript can hold a reference to a `JsBox`, but the contents are opaque
    fn js_open(mut cx: FunctionContext) -> JsResult<JsBox<PlatformWrapper>> {
        let drive_wrapper =
            PlatformWrapper::new(&mut cx).or_else(|err| cx.throw_error(err.to_string()))?;

        Ok(cx.boxed(drive_wrapper))
    }

    /// Sends a message to the DB thread to stop the thread and dispose the
    /// groveDb instance owned by it, then calls js callback passed as a first
    /// argument to the function
    fn js_close(mut cx: FunctionContext) -> JsResult<JsUndefined> {
        let js_callback = cx.argument::<JsFunction>(0)?.root(&mut cx);

        let drive = cx
            .this()
            .downcast_or_throw::<JsBox<PlatformWrapper>, _>(&mut cx)?;

        drive
            .close(|channel| {
                channel.send(move |mut task_context| {
                    let callback = js_callback.into_inner(&mut task_context);
                    let this = task_context.undefined();
                    let callback_arguments: Vec<Handle<JsValue>> =
                        vec![task_context.null().upcast()];

                    callback.call(&mut task_context, this, callback_arguments)?;

                    Ok(())
                });
            })
            .or_else(|err| cx.throw_error(err.to_string()))?;

        Ok(cx.undefined())
    }

    fn js_create_initial_state_structure(mut cx: FunctionContext) -> JsResult<JsUndefined> {
        let js_transaction = cx.argument::<JsValue>(0)?;

        let maybe_boxed_transaction = if !js_transaction.is_a::<JsUndefined, _>(&mut cx) {
            Some(
                js_transaction
                    .downcast_or_throw::<JsBox<PlatformWrapperTransaction>, _>(&mut cx)?,
            )
        } else {
            None
        };

        let js_callback = cx.argument::<JsFunction>(1)?.root(&mut cx);

        let drive = cx
            .this()
            .downcast_or_throw::<JsBox<PlatformWrapper>, _>(&mut cx)?;

        drive
            .send_to_drive_thread(move |platform: &Platform, channel| {
                let maybe_transaction =
                    maybe_boxed_transaction.map(|boxed_transaction| &boxed_transaction.unwrap());

                platform
                    .drive
                    .create_initial_state_structure(maybe_transaction)
                    .expect("create_root_tree should not fail");

                channel.send(move |mut task_context| {
                    let callback = js_callback.into_inner(&mut task_context);
                    let this = task_context.undefined();
                    let callback_arguments: Vec<Handle<JsValue>> =
                        vec![task_context.null().upcast()];

                    callback.call(&mut task_context, this, callback_arguments)?;

                    Ok(())
                });
            })
            .or_else(|err| cx.throw_error(err.to_string()))?;

        Ok(cx.undefined())
    }
    //
    // fn js_apply_contract(mut cx: FunctionContext) -> JsResult<JsUndefined> {
    //     let js_contract_cbor = cx.argument::<JsBuffer>(0)?;
    //     let js_block_time = cx.argument::<JsDate>(1)?;
    //     let js_apply = cx.argument::<JsBoolean>(2)?;
    //     let js_transaction = cx.argument::<JsValue>(3)?;
    //
    //     let maybe_transaction = if js_transaction.is_a::<JsUndefined, _>(&mut cx) {
    //         None
    //     } else {
    //         Some(
    //             js_transaction
    //                 .downcast_or_throw::<JsBox<PlatformWrapperTransaction>, _>(&mut cx)?
    //                 .deref()
    //                 .deref()
    //                 .clone(),
    //         )
    //     };
    //
    //     let js_callback = cx.argument::<JsFunction>(4)?.root(&mut cx);
    //
    //     let drive = cx
    //         .this()
    //         .downcast_or_throw::<JsBox<PlatformWrapper>, _>(&mut cx)?;
    //
    //     let contract_cbor = converter::js_buffer_to_vec_u8(js_contract_cbor, &mut cx);
    //     let apply = js_apply.value(&mut cx);
    //     let block_time = js_block_time.value(&mut cx);
    //
    //     drive
    //         .send_to_drive_thread(move |platform: &Platform, channel| {
    //             let result = platform.drive.apply_contract_cbor(
    //                 contract_cbor,
    //                 None,
    //                 block_time,
    //                 apply,
    //                 StorageFlags::default(),
    //                 maybe_transaction.map(|wrapper| wrapper.deref()),
    //             );
    //
    //             channel.send(move |mut task_context| {
    //                 let callback = js_callback.into_inner(&mut task_context);
    //                 let this = task_context.undefined();
    //
    //                 let callback_arguments: Vec<Handle<JsValue>> = match result {
    //                     Ok((storage_fee, processing_fee)) => {
    //                         let js_array: Handle<JsArray> = task_context.empty_array();
    //
    //                         let storage_fee_value =
    //                             task_context.number(storage_fee as f64).upcast::<JsValue>();
    //                         let processing_fee_value = task_context
    //                             .number(processing_fee as f64)
    //                             .upcast::<JsValue>();
    //
    //                         js_array.set(&mut task_context, 0, storage_fee_value)?;
    //                         js_array.set(&mut task_context, 1, processing_fee_value)?;
    //
    //                         // First parameter of JS callbacks is error, which is null in this case
    //                         vec![task_context.null().upcast(), js_array.upcast()]
    //                     }
    //
    //                     // Convert the error to a JavaScript exception on failure
    //                     Err(err) => vec![task_context.error(err.to_string())?.upcast()],
    //                 };
    //
    //                 callback.call(&mut task_context, this, callback_arguments)?;
    //
    //                 Ok(())
    //             });
    //         })
    //         .or_else(|err| cx.throw_error(err.to_string()))?;
    //
    //     Ok(cx.undefined())
    // }
    //
    // fn js_add_document_for_contract_cbor(mut cx: FunctionContext) -> JsResult<JsUndefined> {
    //     let js_document_cbor = cx.argument::<JsBuffer>(0)?;
    //     let js_contract_cbor = cx.argument::<JsBuffer>(1)?;
    //     let js_document_type_name = cx.argument::<JsString>(2)?;
    //     let js_owner_id = cx.argument::<JsBuffer>(3)?;
    //     let js_override_document = cx.argument::<JsBoolean>(4)?;
    //     let js_block_time = cx.argument::<JsDate>(5)?;
    //     let js_apply = cx.argument::<JsBoolean>(6)?;
    //     let js_transaction = cx.argument::<JsValue>(7)?;
    //
    //     let maybe_transaction = if js_transaction.is_a::<JsUndefined, _>(&mut cx) {
    //         None
    //     } else {
    //         Some(
    //             js_transaction
    //                 .downcast_or_throw::<JsBox<PlatformWrapperTransaction>, _>(&mut cx)?
    //                 .deref()
    //                 .deref()
    //                 .clone(),
    //         )
    //     };
    //
    //     let js_callback = cx.argument::<JsFunction>(8)?.root(&mut cx);
    //
    //     let drive = cx
    //         .this()
    //         .downcast_or_throw::<JsBox<PlatformWrapper>, _>(&mut cx)?;
    //
    //     let document_cbor = converter::js_buffer_to_vec_u8(js_document_cbor, &mut cx);
    //     let contract_cbor = converter::js_buffer_to_vec_u8(js_contract_cbor, &mut cx);
    //     let document_type_name = js_document_type_name.value(&mut cx);
    //     let owner_id = converter::js_buffer_to_vec_u8(js_owner_id, &mut cx);
    //     let override_document = js_override_document.value(&mut cx);
    //     let block_time = js_block_time.value(&mut cx);
    //     let apply = js_apply.value(&mut cx);
    //
    //     drive
    //         .send_to_drive_thread(move |platform: &Platform, channel| {
    //             let result = platform
    //                 .drive
    //                 .add_serialized_document_for_serialized_contract(
    //                     &document_cbor,
    //                     &contract_cbor,
    //                     &document_type_name,
    //                     Some(&owner_id),
    //                     override_document,
    //                     block_time,
    //                     apply,
    //                     StorageFlags::default(),
    //                     maybe_transaction.map(|wrapper| wrapper.deref()),
    //                 );
    //
    //             channel.send(move |mut task_context| {
    //                 let callback = js_callback.into_inner(&mut task_context);
    //                 let this = task_context.undefined();
    //
    //                 let callback_arguments: Vec<Handle<JsValue>> = match result {
    //                     Ok((storage_fee, processing_fee)) => {
    //                         let js_array: Handle<JsArray> = task_context.empty_array();
    //
    //                         let storage_fee_value =
    //                             task_context.number(storage_fee as f64).upcast::<JsValue>();
    //                         let processing_fee_value = task_context
    //                             .number(processing_fee as f64)
    //                             .upcast::<JsValue>();
    //
    //                         js_array.set(&mut task_context, 0, storage_fee_value)?;
    //                         js_array.set(&mut task_context, 1, processing_fee_value)?;
    //
    //                         // First parameter of JS callbacks is error, which is null in this case
    //                         vec![task_context.null().upcast(), js_array.upcast()]
    //                     }
    //
    //                     // Convert the error to a JavaScript exception on failure
    //                     Err(err) => vec![task_context.error(err.to_string())?.upcast()],
    //                 };
    //
    //                 callback.call(&mut task_context, this, callback_arguments)?;
    //
    //                 Ok(())
    //             });
    //         })
    //         .or_else(|err| cx.throw_error(err.to_string()))?;
    //
    //     Ok(cx.undefined())
    // }
    //
    // fn js_update_document_for_contract_cbor(mut cx: FunctionContext) -> JsResult<JsUndefined> {
    //     let js_document_cbor = cx.argument::<JsBuffer>(0)?;
    //     let js_contract_cbor = cx.argument::<JsBuffer>(1)?;
    //     let js_document_type_name = cx.argument::<JsString>(2)?;
    //     let js_owner_id = cx.argument::<JsBuffer>(3)?;
    //     let js_block_time = cx.argument::<JsDate>(4)?;
    //     let js_apply = cx.argument::<JsBoolean>(5)?;
    //     let js_transaction = cx.argument::<JsValue>(6)?;
    //
    //     let maybe_transaction = if js_transaction.is_a::<JsUndefined, _>(&mut cx) {
    //         None
    //     } else {
    //         Some(
    //             js_transaction
    //                 .downcast_or_throw::<JsBox<PlatformWrapperTransaction>, _>(&mut cx)?
    //                 .deref()
    //                 .deref()
    //                 .clone(),
    //         )
    //     };
    //
    //     let js_callback = cx.argument::<JsFunction>(7)?.root(&mut cx);
    //
    //     let drive = cx
    //         .this()
    //         .downcast_or_throw::<JsBox<PlatformWrapper>, _>(&mut cx)?;
    //
    //     let document_cbor = converter::js_buffer_to_vec_u8(js_document_cbor, &mut cx);
    //     let contract_cbor = converter::js_buffer_to_vec_u8(js_contract_cbor, &mut cx);
    //     let document_type_name = js_document_type_name.value(&mut cx);
    //     let owner_id = converter::js_buffer_to_vec_u8(js_owner_id, &mut cx);
    //     let block_time = js_block_time.value(&mut cx);
    //     let apply = js_apply.value(&mut cx);
    //
    //     drive
    //         .send_to_drive_thread(move |platform: &Platform, channel| {
    //             let result = platform.drive.update_document_for_contract_cbor(
    //                 &document_cbor,
    //                 &contract_cbor,
    //                 &document_type_name,
    //                 Some(&owner_id),
    //                 block_time,
    //                 apply,
    //                 StorageFlags::default(),
    //                 maybe_transaction.map(|wrapper| wrapper.deref()),
    //             );
    //
    //             channel.send(move |mut task_context| {
    //                 let callback = js_callback.into_inner(&mut task_context);
    //                 let this = task_context.undefined();
    //
    //                 let callback_arguments: Vec<Handle<JsValue>> = match result {
    //                     Ok((storage_fee, processing_fee)) => {
    //                         let js_array: Handle<JsArray> = task_context.empty_array();
    //
    //                         let storage_fee_value =
    //                             task_context.number(storage_fee as f64).upcast::<JsValue>();
    //                         let processing_fee_value = task_context
    //                             .number(processing_fee as f64)
    //                             .upcast::<JsValue>();
    //
    //                         js_array.set(&mut task_context, 0, storage_fee_value)?;
    //                         js_array.set(&mut task_context, 1, processing_fee_value)?;
    //
    //                         // First parameter of JS callbacks is error, which is null in this case
    //                         vec![task_context.null().upcast(), js_array.upcast()]
    //                     }
    //
    //                     // Convert the error to a JavaScript exception on failure
    //                     Err(err) => vec![task_context.error(err.to_string())?.upcast()],
    //                 };
    //
    //                 callback.call(&mut task_context, this, callback_arguments)?;
    //
    //                 Ok(())
    //             });
    //         })
    //         .or_else(|err| cx.throw_error(err.to_string()))?;
    //
    //     Ok(cx.undefined())
    // }
    //
    // fn js_delete_document_for_contract_cbor(mut cx: FunctionContext) -> JsResult<JsUndefined> {
    //     let js_document_id = cx.argument::<JsBuffer>(0)?;
    //     let js_contract_cbor = cx.argument::<JsBuffer>(1)?;
    //     let js_document_type_name = cx.argument::<JsString>(2)?;
    //     let js_apply = cx.argument::<JsBoolean>(3)?;
    //     let js_transaction = cx.argument::<JsValue>(0)?;
    //
    //     let maybe_transaction = if js_transaction.is_a::<JsUndefined, _>(&mut cx) {
    //         None
    //     } else {
    //         Some(
    //             js_transaction
    //                 .downcast_or_throw::<JsBox<PlatformWrapperTransaction>, _>(&mut cx)?
    //                 .deref()
    //                 .deref()
    //                 .clone(),
    //         )
    //     };
    //
    //     let js_callback = cx.argument::<JsFunction>(5)?.root(&mut cx);
    //
    //     let drive = cx
    //         .this()
    //         .downcast_or_throw::<JsBox<PlatformWrapper>, _>(&mut cx)?;
    //
    //     let document_id = converter::js_buffer_to_vec_u8(js_document_id, &mut cx);
    //     let contract_cbor = converter::js_buffer_to_vec_u8(js_contract_cbor, &mut cx);
    //     let document_type_name = js_document_type_name.value(&mut cx);
    //     let apply = js_apply.value(&mut cx);
    //
    //     drive
    //         .send_to_drive_thread(move |platform: &Platform, channel| {
    //             let result = platform.drive.delete_document_for_contract_cbor(
    //                 &document_id,
    //                 &contract_cbor,
    //                 &document_type_name,
    //                 None,
    //                 apply,
    //                 maybe_transaction.map(|wrapper| wrapper.deref()),
    //             );
    //
    //             channel.send(move |mut task_context| {
    //                 let callback = js_callback.into_inner(&mut task_context);
    //                 let this = task_context.undefined();
    //
    //                 let callback_arguments: Vec<Handle<JsValue>> = match result {
    //                     Ok((storage_fee, processing_fee)) => {
    //                         let js_array: Handle<JsArray> = task_context.empty_array();
    //
    //                         let storage_fee_value =
    //                             task_context.number(storage_fee as f64).upcast::<JsValue>();
    //                         let processing_fee_value = task_context
    //                             .number(processing_fee as f64)
    //                             .upcast::<JsValue>();
    //
    //                         js_array.set(&mut task_context, 0, storage_fee_value)?;
    //                         js_array.set(&mut task_context, 1, processing_fee_value)?;
    //
    //                         // First parameter of JS callbacks is error, which is null in this case
    //                         vec![task_context.null().upcast(), js_array.upcast()]
    //                     }
    //
    //                     // Convert the error to a JavaScript exception on failure
    //                     Err(err) => vec![task_context.error(err.to_string())?.upcast()],
    //                 };
    //
    //                 callback.call(&mut task_context, this, callback_arguments)?;
    //
    //                 Ok(())
    //             });
    //         })
    //         .or_else(|err| cx.throw_error(err.to_string()))?;
    //
    //     Ok(cx.undefined())
    // }
    //
    // fn js_insert_identity_cbor(mut cx: FunctionContext) -> JsResult<JsUndefined> {
    //     let js_identity_cbor = cx.argument::<JsBuffer>(0)?;
    //     let js_apply = cx.argument::<JsBoolean>(1)?;
    //     let js_transaction = cx.argument::<JsValue>(2)?;
    //
    //     let maybe_transaction = if js_transaction.is_a::<JsUndefined, _>(&mut cx) {
    //         None
    //     } else {
    //         Some(
    //             js_transaction
    //                 .downcast_or_throw::<JsBox<PlatformWrapperTransaction>, _>(&mut cx)?
    //                 .deref()
    //                 .deref()
    //                 .clone(),
    //         )
    //     };
    //
    //     let js_callback = cx.argument::<JsFunction>(3)?.root(&mut cx);
    //
    //     let drive = cx
    //         .this()
    //         .downcast_or_throw::<JsBox<PlatformWrapper>, _>(&mut cx)?;
    //
    //     let identity_cbor = converter::js_buffer_to_vec_u8(js_identity_cbor, &mut cx);
    //     let apply = js_apply.value(&mut cx);
    //
    //     let identity =
    //         Identity::from_buffer(identity_cbor).or_else(|e| cx.throw_error(e.to_string()))?;
    //
    //     drive
    //         .send_to_drive_thread(move |platform: &Platform, channel| {
    //             let result = platform.drive.insert_identity(
    //                 identity,
    //                 apply,
    //                 StorageFlags::default(),
    //                 maybe_transaction.map(|wrapper| wrapper.deref()),
    //             );
    //
    //             channel.send(move |mut task_context| {
    //                 let callback = js_callback.into_inner(&mut task_context);
    //                 let this = task_context.undefined();
    //
    //                 let callback_arguments: Vec<Handle<JsValue>> = match result {
    //                     Ok((storage_fee, processing_fee)) => {
    //                         let js_array: Handle<JsArray> = task_context.empty_array();
    //
    //                         let storage_fee_value =
    //                             task_context.number(storage_fee as f64).upcast::<JsValue>();
    //                         let processing_fee_value = task_context
    //                             .number(processing_fee as f64)
    //                             .upcast::<JsValue>();
    //
    //                         js_array.set(&mut task_context, 0, storage_fee_value)?;
    //                         js_array.set(&mut task_context, 1, processing_fee_value)?;
    //
    //                         // First parameter of JS callbacks is error, which is null in this case
    //                         vec![task_context.null().upcast(), js_array.upcast()]
    //                     }
    //
    //                     // Convert the error to a JavaScript exception on failure
    //                     Err(err) => vec![task_context.error(err.to_string())?.upcast()],
    //                 };
    //
    //                 callback.call(&mut task_context, this, callback_arguments)?;
    //
    //                 Ok(())
    //             });
    //         })
    //         .or_else(|err| cx.throw_error(err.to_string()))?;
    //
    //     Ok(cx.undefined())
    // }
    //
    // fn js_query_documents(mut cx: FunctionContext) -> JsResult<JsUndefined> {
    //     let js_query_cbor = cx.argument::<JsBuffer>(0)?;
    //     let js_contract_id = cx.argument::<JsBuffer>(1)?;
    //     let js_document_type_name = cx.argument::<JsString>(2)?;
    //     let js_transaction = cx.argument::<JsValue>(3)?;
    //
    //     let maybe_transaction = if js_transaction.is_a::<JsUndefined, _>(&mut cx) {
    //         None
    //     } else {
    //         Some(
    //             js_transaction
    //                 .downcast_or_throw::<JsBox<PlatformWrapperTransaction>, _>(&mut cx)?
    //                 .deref()
    //                 .deref()
    //                 .clone(),
    //         )
    //     };
    //
    //     let js_callback = cx.argument::<JsFunction>(4)?.root(&mut cx);
    //
    //     let drive = cx
    //         .this()
    //         .downcast_or_throw::<JsBox<PlatformWrapper>, _>(&mut cx)?;
    //
    //     let query_cbor = converter::js_buffer_to_vec_u8(js_query_cbor, &mut cx);
    //     let contract_id = converter::js_buffer_to_vec_u8(js_contract_id, &mut cx);
    //     let document_type_name = js_document_type_name.value(&mut cx);
    //
    //     drive
    //         .send_to_drive_thread(move |platform: &Platform, channel| {
    //             let result = platform.drive.query_documents(
    //                 &query_cbor,
    //                 <[u8; 32]>::try_from(contract_id).unwrap(),
    //                 document_type_name.as_str(),
    //                 maybe_transaction.map(|wrapper| wrapper.deref()),
    //             );
    //
    //             channel.send(move |mut task_context| {
    //                 let callback = js_callback.into_inner(&mut task_context);
    //                 let this = task_context.undefined();
    //                 let callback_arguments: Vec<Handle<JsValue>> = match result {
    //                     Ok((value, skipped, cost)) => {
    //                         let js_array: Handle<JsArray> = task_context.empty_array();
    //                         let js_vecs = converter::nested_vecs_to_js(value, &mut task_context)?;
    //                         let js_num = task_context.number(skipped).upcast::<JsValue>();
    //                         let js_cost = task_context.number(cost as f64).upcast::<JsValue>();
    //
    //                         js_array.set(&mut task_context, 0, js_vecs)?;
    //                         js_array.set(&mut task_context, 1, js_num)?;
    //                         js_array.set(&mut task_context, 2, js_cost)?;
    //
    //                         vec![task_context.null().upcast(), js_array.upcast()]
    //                     }
    //
    //                     // Convert the error to a JavaScript exception on failure
    //                     Err(err) => vec![task_context.error(err.to_string())?.upcast()],
    //                 };
    //
    //                 callback.call(&mut task_context, this, callback_arguments)?;
    //
    //                 Ok(())
    //             });
    //         })
    //         .or_else(|err| cx.throw_error(err.to_string()))?;
    //
    //     Ok(cx.undefined())
    // }
    //
    // fn js_prove_documents_query(mut cx: FunctionContext) -> JsResult<JsUndefined> {
    //     let js_query_cbor = cx.argument::<JsBuffer>(0)?;
    //     let js_contract_id = cx.argument::<JsBuffer>(1)?;
    //     let js_document_type_name = cx.argument::<JsString>(2)?;
    //     let js_transaction = cx.argument::<JsValue>(3)?;
    //
    //     let maybe_transaction = if js_transaction.is_a::<JsUndefined, _>(&mut cx) {
    //         None
    //     } else {
    //         Some(
    //             js_transaction
    //                 .downcast_or_throw::<JsBox<PlatformWrapperTransaction>, _>(&mut cx)?
    //                 .deref()
    //                 .deref()
    //                 .clone(),
    //         )
    //     };
    //
    //     let js_callback = cx.argument::<JsFunction>(4)?.root(&mut cx);
    //
    //     let drive = cx
    //         .this()
    //         .downcast_or_throw::<JsBox<PlatformWrapper>, _>(&mut cx)?;
    //
    //     let query_cbor = converter::js_buffer_to_vec_u8(js_query_cbor, &mut cx);
    //     let contract_id = converter::js_buffer_to_vec_u8(js_contract_id, &mut cx);
    //     let document_type_name = js_document_type_name.value(&mut cx);
    //
    //     drive
    //         .send_to_drive_thread(move |platform: &Platform, channel| {
    //             let result = platform.drive.query_documents_as_grove_proof(
    //                 &query_cbor,
    //                 <[u8; 32]>::try_from(contract_id).unwrap(),
    //                 document_type_name.as_str(),
    //                 maybe_transaction.map(|wrapper| wrapper.deref()),
    //             );
    //
    //             channel.send(move |mut task_context| {
    //                 let callback = js_callback.into_inner(&mut task_context);
    //                 let this = task_context.undefined();
    //                 let callback_arguments: Vec<Handle<JsValue>> = match result {
    //                     Ok((proof, processing_cost)) => {
    //                         let js_array: Handle<JsArray> = task_context.empty_array();
    //                         let js_buffer = JsBuffer::external(&mut task_context, proof);
    //                         let js_processing_cost = task_context.number(processing_cost as f64);
    //
    //                         js_array.set(&mut task_context, 0, js_buffer)?;
    //                         js_array.set(&mut task_context, 1, js_processing_cost)?;
    //
    //                         vec![task_context.null().upcast(), js_array.upcast()]
    //                     }
    //
    //                     // Convert the error to a JavaScript exception on failure
    //                     Err(err) => vec![task_context.error(err.to_string())?.upcast()],
    //                 };
    //
    //                 callback.call(&mut task_context, this, callback_arguments)?;
    //
    //                 Ok(())
    //             });
    //         })
    //         .or_else(|err| cx.throw_error(err.to_string()))?;
    //
    //     Ok(cx.undefined())
    // }
    //
    // fn js_grove_db_start_transaction(mut cx: FunctionContext) -> JsResult<JsUndefined> {
    //     let js_callback = cx.argument::<JsFunction>(0)?.root(&mut cx);
    //
    //     let db = cx
    //         .this()
    //         .downcast_or_throw::<JsBox<PlatformWrapper>, _>(&mut cx)?;
    //
    //     db.send_to_drive_thread(|platform, channel| {
    //         let transaction = platform.drive.grove.start_transaction();
    //
    //         let transaction = PlatformWrapperTransaction(transaction);
    //
    //         channel.send(move |mut task_context| {
    //             let callback = js_callback.into_inner(&mut task_context);
    //             let this = task_context.undefined();
    //             let callback_arguments: Vec<Handle<JsValue>> = vec![
    //                 task_context.null().upcast(),
    //                 task_context.boxed(transaction).upcast(),
    //             ];
    //
    //             callback.call(&mut task_context, this, callback_arguments)?;
    //
    //             Ok(())
    //         });
    //     })
    //     .or_else(|err| cx.throw_error(err.to_string()))?;
    //
    //     Ok(cx.undefined())
    // }
    //
    // fn js_grove_db_commit_transaction(mut cx: FunctionContext) -> JsResult<JsUndefined> {
    //     let js_transaction = cx.argument::<JsValue>(0)?;
    //
    //     let maybe_transaction = if js_transaction.is_a::<JsUndefined, _>(&mut cx) {
    //         None
    //     } else {
    //         Some(
    //             js_transaction
    //                 .downcast_or_throw::<JsBox<PlatformWrapperTransaction>, _>(&mut cx)?
    //                 .deref()
    //                 .deref()
    //                 .clone(),
    //         )
    //     };
    //
    //     if maybe_transaction.is_none() {
    //         cx.throw_type_error("transaction is undefined")?;
    //     }
    //
    //     let js_callback = cx.argument::<JsFunction>(1)?.root(&mut cx);
    //
    //     let db = cx
    //         .this()
    //         .downcast_or_throw::<JsBox<PlatformWrapper>, _>(&mut cx)?;
    //
    //     db.send_to_drive_thread(move |platform, channel| {
    //         platform
    //             .drive
    //             .grove
    //             .commit_transaction(maybe_transaction.unwrap().unwrap())
    //             .unwrap()
    //             .unwrap();
    //
    //         channel.send(move |mut task_context| {
    //             let callback = js_callback.into_inner(&mut task_context);
    //             let this = task_context.undefined();
    //             let callback_arguments: Vec<Handle<JsValue>> = vec![task_context.null().upcast()];
    //
    //             callback.call(&mut task_context, this, callback_arguments)?;
    //
    //             Ok(())
    //         });
    //     })
    //     .or_else(|err| cx.throw_error(err.to_string()))?;
    //
    //     Ok(cx.undefined())
    // }
    //
    // fn js_grove_db_rollback_transaction(mut cx: FunctionContext) -> JsResult<JsUndefined> {
    //     let js_transaction = cx.argument::<JsValue>(0)?;
    //
    //     let maybe_transaction = if js_transaction.is_a::<JsUndefined, _>(&mut cx) {
    //         None
    //     } else {
    //         Some(
    //             js_transaction
    //                 .downcast_or_throw::<JsBox<PlatformWrapperTransaction>, _>(&mut cx)?
    //                 .deref()
    //                 .deref()
    //                 .clone(),
    //         )
    //     };
    //
    //     if maybe_transaction.is_none() {
    //         cx.throw_type_error("transaction is undefined")?;
    //     }
    //
    //     let js_callback = cx.argument::<JsFunction>(1)?.root(&mut cx);
    //
    //     let db = cx
    //         .this()
    //         .downcast_or_throw::<JsBox<PlatformWrapper>, _>(&mut cx)?;
    //
    //     db.send_to_drive_thread(|platform, channel| {
    //         platform
    //             .drive
    //             .grove
    //             .rollback_transaction(maybe_transaction.unwrap().deref())
    //             .unwrap();
    //
    //         channel.send(move |mut task_context| {
    //             let callback = js_callback.into_inner(&mut task_context);
    //             let this = task_context.undefined();
    //             let callback_arguments: Vec<Handle<JsValue>> = vec![task_context.null().upcast()];
    //
    //             callback.call(&mut task_context, this, callback_arguments)?;
    //
    //             Ok(())
    //         });
    //     })
    //     .or_else(|err| cx.throw_error(err.to_string()))?;
    //
    //     Ok(cx.undefined())
    // }
    //
    // fn js_grove_db_abort_transaction(mut cx: FunctionContext) -> JsResult<JsUndefined> {
    //     let js_transaction = cx.argument::<JsValue>(0)?;
    //
    //     let maybe_transaction = if js_transaction.is_a::<JsUndefined, _>(&mut cx) {
    //         None
    //     } else {
    //         Some(
    //             js_transaction
    //                 .downcast_or_throw::<JsBox<PlatformWrapperTransaction>, _>(&mut cx)?
    //                 .deref()
    //                 .deref()
    //                 .clone(),
    //         )
    //     };
    //
    //     if maybe_transaction.is_none() {
    //         cx.throw_type_error("transaction is undefined")?;
    //     }
    //
    //     let js_callback = cx.argument::<JsFunction>(1)?.root(&mut cx);
    //
    //     let db = cx
    //         .this()
    //         .downcast_or_throw::<JsBox<PlatformWrapper>, _>(&mut cx)?;
    //
    //     db.send_to_drive_thread(|platform, channel| {
    //         drop(maybe_transaction.unwrap().unwrap());
    //
    //         channel.send(move |mut task_context| {
    //             let callback = js_callback.into_inner(&mut task_context);
    //             let this = task_context.undefined();
    //             let callback_arguments: Vec<Handle<JsValue>> = vec![task_context.null().upcast()];
    //
    //             callback.call(&mut task_context, this, callback_arguments)?;
    //
    //             Ok(())
    //         });
    //     })
    //     .or_else(|err| cx.throw_error(err.to_string()))?;
    //
    //     Ok(cx.undefined())
    // }
    //
    // fn js_grove_db_get(mut cx: FunctionContext) -> JsResult<JsUndefined> {
    //     let js_path = cx.argument::<JsArray>(0)?;
    //     let js_key = cx.argument::<JsBuffer>(1)?;
    //     let js_transaction = cx.argument::<JsValue>(2)?;
    //
    //     let maybe_transaction = if js_transaction.is_a::<JsUndefined, _>(&mut cx) {
    //         None
    //     } else {
    //         Some(
    //             js_transaction
    //                 .downcast_or_throw::<JsBox<PlatformWrapperTransaction>, _>(&mut cx)?
    //                 .deref()
    //                 .deref()
    //                 .clone(),
    //         )
    //     };
    //
    //     let js_callback = cx.argument::<JsFunction>(3)?.root(&mut cx);
    //
    //     let path = converter::js_array_of_buffers_to_vec(js_path, &mut cx)?;
    //     let key = converter::js_buffer_to_vec_u8(js_key, &mut cx);
    //
    //     // Get the `this` value as a `JsBox<Database>`
    //     let db = cx
    //         .this()
    //         .downcast_or_throw::<JsBox<PlatformWrapper>, _>(&mut cx)?;
    //
    //     db.send_to_drive_thread(move |platform: &Platform, channel| {
    //         let grove_db = &platform.drive.grove;
    //         let path_slice = path.iter().map(|fragment| fragment.as_slice());
    //         let result = grove_db
    //             .get(
    //                 path_slice,
    //                 &key,
    //                 maybe_transaction.map(|wrapper| wrapper.deref()),
    //             )
    //             .unwrap();
    //
    //         channel.send(move |mut task_context| {
    //             let callback = js_callback.into_inner(&mut task_context);
    //             let this = task_context.undefined();
    //             let callback_arguments: Vec<Handle<JsValue>> = match result {
    //                 Ok(element) => {
    //                     // First parameter of JS callbacks is error, which is null in this case
    //                     vec![
    //                         task_context.null().upcast(),
    //                         converter::element_to_js_object(element, &mut task_context)?,
    //                     ]
    //                 }
    //
    //                 // Convert the error to a JavaScript exception on failure
    //                 Err(err) => vec![task_context.error(err.to_string())?.upcast()],
    //             };
    //
    //             callback.call(&mut task_context, this, callback_arguments)?;
    //
    //             Ok(())
    //         });
    //     })
    //     .or_else(|err| cx.throw_error(err.to_string()))?;
    //
    //     // The result is returned through the callback, not through direct return
    //     Ok(cx.undefined())
    // }
    //
    // fn js_grove_db_insert(mut cx: FunctionContext) -> JsResult<JsUndefined> {
    //     let js_path = cx.argument::<JsArray>(0)?;
    //     let js_key = cx.argument::<JsBuffer>(1)?;
    //     let js_element = cx.argument::<JsObject>(2)?;
    //     let js_transaction = cx.argument::<JsValue>(3)?;
    //
    //     let maybe_transaction = if js_transaction.is_a::<JsUndefined, _>(&mut cx) {
    //         None
    //     } else {
    //         Some(
    //             js_transaction
    //                 .downcast_or_throw::<JsBox<PlatformWrapperTransaction>, _>(&mut cx)?
    //                 .deref()
    //                 .deref()
    //                 .clone(),
    //         )
    //     };
    //
    //     let js_callback = cx.argument::<JsFunction>(4)?.root(&mut cx);
    //
    //     let path = converter::js_array_of_buffers_to_vec(js_path, &mut cx)?;
    //     let key = converter::js_buffer_to_vec_u8(js_key, &mut cx);
    //     let element = converter::js_object_to_element(js_element, &mut cx)?;
    //
    //     // Get the `this` value as a `JsBox<Database>`
    //     let db = cx
    //         .this()
    //         .downcast_or_throw::<JsBox<PlatformWrapper>, _>(&mut cx)?;
    //
    //     db.send_to_drive_thread(move |platform: &Platform, channel| {
    //         let grove_db = &platform.drive.grove;
    //         let path_slice = path.iter().map(|fragment| fragment.as_slice());
    //         let result = grove_db
    //             .insert(
    //                 path_slice,
    //                 &key,
    //                 element,
    //                 maybe_transaction.map(|wrapper| wrapper.deref()),
    //             )
    //             .unwrap();
    //
    //         channel.send(move |mut task_context| {
    //             let callback = js_callback.into_inner(&mut task_context);
    //             let this = task_context.undefined();
    //
    //             let callback_arguments: Vec<Handle<JsValue>> = match result {
    //                 Ok(_) => vec![task_context.null().upcast()],
    //                 Err(err) => vec![task_context.error(err.to_string())?.upcast()],
    //             };
    //
    //             callback.call(&mut task_context, this, callback_arguments)?;
    //             Ok(())
    //         });
    //     })
    //     .or_else(|err| cx.throw_error(err.to_string()))?;
    //
    //     Ok(cx.undefined())
    // }
    //
    // fn js_grove_db_insert_if_not_exists(mut cx: FunctionContext) -> JsResult<JsUndefined> {
    //     let js_path = cx.argument::<JsArray>(0)?;
    //     let js_key = cx.argument::<JsBuffer>(1)?;
    //     let js_element = cx.argument::<JsObject>(2)?;
    //     let js_transaction = cx.argument::<JsValue>(3)?;
    //
    //     let maybe_transaction = if js_transaction.is_a::<JsUndefined, _>(&mut cx) {
    //         None
    //     } else {
    //         Some(
    //             js_transaction
    //                 .downcast_or_throw::<JsBox<PlatformWrapperTransaction>, _>(&mut cx)?
    //                 .deref()
    //                 .deref()
    //                 .clone(),
    //         )
    //     };
    //
    //     let js_callback = cx.argument::<JsFunction>(4)?.root(&mut cx);
    //
    //     let path = converter::js_array_of_buffers_to_vec(js_path, &mut cx)?;
    //     let key = converter::js_buffer_to_vec_u8(js_key, &mut cx);
    //     let element = converter::js_object_to_element(js_element, &mut cx)?;
    //
    //     // Get the `this` value as a `JsBox<Database>`
    //     let db = cx
    //         .this()
    //         .downcast_or_throw::<JsBox<PlatformWrapper>, _>(&mut cx)?;
    //
    //     db.send_to_drive_thread(move |platform: &Platform, channel| {
    //         let grove_db = &platform.drive.grove;
    //
    //         let path_slice: Vec<&[u8]> = path.iter().map(|fragment| fragment.as_slice()).collect();
    //         let result = grove_db
    //             .insert_if_not_exists(
    //                 path_slice,
    //                 key.as_slice(),
    //                 element,
    //                 maybe_transaction.map(|wrapper| wrapper.deref()),
    //             )
    //             .unwrap();
    //
    //         channel.send(move |mut task_context| {
    //             let callback = js_callback.into_inner(&mut task_context);
    //             let this = task_context.undefined();
    //             let callback_arguments: Vec<Handle<JsValue>> = match result {
    //                 Ok(is_inserted) => vec![
    //                     task_context.null().upcast(),
    //                     task_context
    //                         .boolean(is_inserted)
    //                         .as_value(&mut task_context),
    //                 ],
    //                 Err(err) => vec![task_context.error(err.to_string())?.upcast()],
    //             };
    //
    //             callback.call(&mut task_context, this, callback_arguments)?;
    //
    //             Ok(())
    //         });
    //     })
    //     .or_else(|err| cx.throw_error(err.to_string()))?;
    //
    //     Ok(cx.undefined())
    // }
    //
    // fn js_grove_db_put_aux(mut cx: FunctionContext) -> JsResult<JsUndefined> {
    //     let js_key = cx.argument::<JsBuffer>(0)?;
    //     let js_value = cx.argument::<JsBuffer>(1)?;
    //     let js_transaction = cx.argument::<JsValue>(2)?;
    //
    //     let maybe_transaction = if js_transaction.is_a::<JsUndefined, _>(&mut cx) {
    //         None
    //     } else {
    //         Some(
    //             js_transaction
    //                 .downcast_or_throw::<JsBox<PlatformWrapperTransaction>, _>(&mut cx)?
    //                 .deref()
    //                 .deref()
    //                 .clone(),
    //         )
    //     };
    //
    //     let js_callback = cx.argument::<JsFunction>(3)?.root(&mut cx);
    //
    //     let key = converter::js_buffer_to_vec_u8(js_key, &mut cx);
    //     let value = converter::js_buffer_to_vec_u8(js_value, &mut cx);
    //
    //     let db = cx
    //         .this()
    //         .downcast_or_throw::<JsBox<PlatformWrapper>, _>(&mut cx)?;
    //
    //     db.send_to_drive_thread(move |platform: &Platform, channel| {
    //         let grove_db = &platform.drive.grove;
    //
    //         let result = grove_db
    //             .put_aux(
    //                 &key,
    //                 &value,
    //                 maybe_transaction.map(|wrapper| wrapper.deref()),
    //             )
    //             .unwrap();
    //
    //         channel.send(move |mut task_context| {
    //             let callback = js_callback.into_inner(&mut task_context);
    //             let this = task_context.undefined();
    //             let callback_arguments: Vec<Handle<JsValue>> = match result {
    //                 Ok(()) => {
    //                     vec![task_context.null().upcast()]
    //                 }
    //
    //                 // Convert the error to a JavaScript exception on failure
    //                 Err(err) => vec![task_context.error(err.to_string())?.upcast()],
    //             };
    //
    //             callback.call(&mut task_context, this, callback_arguments)?;
    //
    //             Ok(())
    //         });
    //     })
    //     .or_else(|err| cx.throw_error(err.to_string()))?;
    //
    //     // The result is returned through the callback, not through direct return
    //     Ok(cx.undefined())
    // }
    //
    // fn js_grove_db_delete_aux(mut cx: FunctionContext) -> JsResult<JsUndefined> {
    //     let js_key = cx.argument::<JsBuffer>(0)?;
    //     let js_transaction = cx.argument::<JsValue>(1)?;
    //
    //     let maybe_transaction = if js_transaction.is_a::<JsUndefined, _>(&mut cx) {
    //         None
    //     } else {
    //         Some(
    //             js_transaction
    //                 .downcast_or_throw::<JsBox<PlatformWrapperTransaction>, _>(&mut cx)?
    //                 .deref()
    //                 .deref()
    //                 .clone(),
    //         )
    //     };
    //
    //     let js_callback = cx.argument::<JsFunction>(2)?.root(&mut cx);
    //
    //     let key = converter::js_buffer_to_vec_u8(js_key, &mut cx);
    //
    //     let db = cx
    //         .this()
    //         .downcast_or_throw::<JsBox<PlatformWrapper>, _>(&mut cx)?;
    //
    //     db.send_to_drive_thread(move |platform: &Platform, channel| {
    //         let grove_db = &platform.drive.grove;
    //
    //         let result = grove_db
    //             .delete_aux(&key, maybe_transaction.map(|wrapper| wrapper.deref()))
    //             .unwrap();
    //
    //         channel.send(move |mut task_context| {
    //             let callback = js_callback.into_inner(&mut task_context);
    //             let this = task_context.undefined();
    //             let callback_arguments: Vec<Handle<JsValue>> = match result {
    //                 Ok(()) => {
    //                     vec![task_context.null().upcast()]
    //                 }
    //
    //                 // Convert the error to a JavaScript exception on failure
    //                 Err(err) => vec![task_context.error(err.to_string())?.upcast()],
    //             };
    //
    //             callback.call(&mut task_context, this, callback_arguments)?;
    //
    //             Ok(())
    //         });
    //     })
    //     .or_else(|err| cx.throw_error(err.to_string()))?;
    //
    //     // The result is returned through the callback, not through direct return
    //     Ok(cx.undefined())
    // }
    //
    // fn js_grove_db_get_aux(mut cx: FunctionContext) -> JsResult<JsUndefined> {
    //     let js_key = cx.argument::<JsBuffer>(0)?;
    //     let js_transaction = cx.argument::<JsValue>(1)?;
    //
    //     let maybe_transaction = if js_transaction.is_a::<JsUndefined, _>(&mut cx) {
    //         None
    //     } else {
    //         Some(
    //             js_transaction
    //                 .downcast_or_throw::<JsBox<PlatformWrapperTransaction>, _>(&mut cx)?
    //                 .deref()
    //                 .deref()
    //                 .clone(),
    //         )
    //     };
    //
    //     let js_callback = cx.argument::<JsFunction>(2)?.root(&mut cx);
    //
    //     let key = converter::js_buffer_to_vec_u8(js_key, &mut cx);
    //
    //     let db = cx
    //         .this()
    //         .downcast_or_throw::<JsBox<PlatformWrapper>, _>(&mut cx)?;
    //
    //     db.send_to_drive_thread(move |platform: &Platform, channel| {
    //         let grove_db = &platform.drive.grove;
    //
    //         let result = grove_db
    //             .get_aux(&key, maybe_transaction.map(|wrapper| wrapper.deref()))
    //             .unwrap();
    //
    //         channel.send(move |mut task_context| {
    //             let callback = js_callback.into_inner(&mut task_context);
    //             let this = task_context.undefined();
    //             let callback_arguments: Vec<Handle<JsValue>> = match result {
    //                 Ok(value) => {
    //                     if let Some(value) = value {
    //                         vec![
    //                             task_context.null().upcast(),
    //                             JsBuffer::external(&mut task_context, value).upcast(),
    //                         ]
    //                     } else {
    //                         vec![task_context.null().upcast(), task_context.null().upcast()]
    //                     }
    //                 }
    //
    //                 // Convert the error to a JavaScript exception on failure
    //                 Err(err) => vec![task_context.error(err.to_string())?.upcast()],
    //             };
    //
    //             callback.call(&mut task_context, this, callback_arguments)?;
    //
    //             Ok(())
    //         });
    //     })
    //     .or_else(|err| cx.throw_error(err.to_string()))?;
    //
    //     // The result is returned through the callback, not through direct return
    //     Ok(cx.undefined())
    // }
    //
    // fn js_grove_db_query(mut cx: FunctionContext) -> JsResult<JsUndefined> {
    //     let js_path_query = cx.argument::<JsObject>(0)?;
    //     let js_transaction = cx.argument::<JsValue>(1)?;
    //
    //     let maybe_transaction = if js_transaction.is_a::<JsUndefined, _>(&mut cx) {
    //         None
    //     } else {
    //         Some(
    //             js_transaction
    //                 .downcast_or_throw::<JsBox<PlatformWrapperTransaction>, _>(&mut cx)?
    //                 .deref()
    //                 .deref()
    //                 .clone(),
    //         )
    //     };
    //
    //     let js_callback = cx.argument::<JsFunction>(2)?.root(&mut cx);
    //
    //     let path_query = converter::js_path_query_to_path_query(js_path_query, &mut cx)?;
    //
    //     let db = cx
    //         .this()
    //         .downcast_or_throw::<JsBox<PlatformWrapper>, _>(&mut cx)?;
    //
    //     db.send_to_drive_thread(move |platform: &Platform, channel| {
    //         let grove_db = &platform.drive.grove;
    //
    //         let result = grove_db
    //             .query(
    //                 &path_query,
    //                 maybe_transaction.map(|wrapper| wrapper.deref()),
    //             )
    //             .unwrap();
    //
    //         channel.send(move |mut task_context| {
    //             let callback = js_callback.into_inner(&mut task_context);
    //             let this = task_context.undefined();
    //             let callback_arguments: Vec<Handle<JsValue>> = match result {
    //                 Ok((value, skipped)) => {
    //                     let js_array: Handle<JsArray> = task_context.empty_array();
    //                     let js_vecs = converter::nested_vecs_to_js(value, &mut task_context)?;
    //                     let js_num = task_context.number(skipped).upcast::<JsValue>();
    //                     js_array.set(&mut task_context, 0, js_vecs)?;
    //                     js_array.set(&mut task_context, 1, js_num)?;
    //
    //                     vec![task_context.null().upcast(), js_array.upcast()]
    //                 }
    //
    //                 // Convert the error to a JavaScript exception on failure
    //                 Err(err) => vec![task_context.error(err.to_string())?.upcast()],
    //             };
    //
    //             callback.call(&mut task_context, this, callback_arguments)?;
    //
    //             Ok(())
    //         });
    //     })
    //     .or_else(|err| cx.throw_error(err.to_string()))?;
    //
    //     // The result is returned through the callback, not through direct return
    //     Ok(cx.undefined())
    // }
    //
    // fn js_grove_db_prove_query(mut cx: FunctionContext) -> JsResult<JsUndefined> {
    //     let js_path_query = cx.argument::<JsObject>(0)?;
    //     let js_transaction = cx.argument::<JsValue>(1)?;
    //
    //     let maybe_transaction = if js_transaction.is_a::<JsUndefined, _>(&mut cx) {
    //         None
    //     } else {
    //         Some(
    //             js_transaction
    //                 .downcast_or_throw::<JsBox<PlatformWrapperTransaction>, _>(&mut cx)?
    //                 .deref()
    //                 .deref()
    //                 .clone(),
    //         )
    //     };
    //
    //     let js_callback = cx.argument::<JsFunction>(2)?.root(&mut cx);
    //
    //     let path_query = converter::js_path_query_to_path_query(js_path_query, &mut cx)?;
    //
    //     let db = cx
    //         .this()
    //         .downcast_or_throw::<JsBox<PlatformWrapper>, _>(&mut cx)?;
    //
    //     db.send_to_drive_thread(move |platform: &Platform, channel| {
    //         let grove_db = &platform.drive.grove;
    //
    //         let result = grove_db
    //             .get_proved_path_query(
    //                 &path_query,
    //                 maybe_transaction.map(|wrapper| wrapper.deref()),
    //             )
    //             .unwrap();
    //
    //         channel.send(move |mut task_context| {
    //             let callback = js_callback.into_inner(&mut task_context);
    //             let this = task_context.undefined();
    //             let callback_arguments: Vec<Handle<JsValue>> = match result {
    //                 Ok(proof) => {
    //                     let js_buffer = JsBuffer::external(&mut task_context, proof.clone());
    //                     let js_value = js_buffer.as_value(&mut task_context);
    //
    //                     vec![task_context.null().upcast(), js_value.upcast()]
    //                 }
    //
    //                 // Convert the error to a JavaScript exception on failure
    //                 Err(err) => vec![task_context.error(err.to_string())?.upcast()],
    //             };
    //
    //             callback.call(&mut task_context, this, callback_arguments)?;
    //
    //             Ok(())
    //         });
    //     })
    //     .or_else(|err| cx.throw_error(err.to_string()))?;
    //
    //     // The result is returned through the callback, not through direct return
    //     Ok(cx.undefined())
    // }
    //
    // fn js_grove_db_prove_query_many(mut cx: FunctionContext) -> JsResult<JsUndefined> {
    //     let js_path_queries = cx.argument::<JsArray>(0)?;
    //     let js_transaction = cx.argument::<JsValue>(1)?;
    //
    //     let maybe_transaction = if js_transaction.is_a::<JsUndefined, _>(&mut cx) {
    //         None
    //     } else {
    //         Some(
    //             js_transaction
    //                 .downcast_or_throw::<JsBox<PlatformWrapperTransaction>, _>(&mut cx)?
    //                 .deref()
    //                 .deref()
    //                 .clone(),
    //         )
    //     };
    //
    //     if maybe_transaction.is_some() {
    //         cx.throw_type_error("transaction is undefined")?;
    //     }
    //
    //     let js_callback = cx.argument::<JsFunction>(2)?.root(&mut cx);
    //
    //     let js_path_queries = js_path_queries.to_vec(&mut cx)?;
    //     let mut path_queries: Vec<PathQuery> = Vec::with_capacity(js_path_queries.len());
    //
    //     for js_path_query in js_path_queries {
    //         let js_path_query = js_path_query.downcast_or_throw::<JsObject, _>(&mut cx)?;
    //         path_queries.push(converter::js_path_query_to_path_query(
    //             js_path_query,
    //             &mut cx,
    //         )?);
    //     }
    //
    //     let db = cx
    //         .this()
    //         .downcast_or_throw::<JsBox<PlatformWrapper>, _>(&mut cx)?;
    //
    //     db.send_to_drive_thread(move |platform: &Platform, channel| {
    //         let grove_db = &platform.drive.grove;
    //
    //         let path_queries = path_queries.iter().map(|path_query| path_query).collect();
    //
    //         let result = grove_db.prove_query_many(path_queries).unwrap();
    //
    //         channel.send(move |mut task_context| {
    //             let this = task_context.undefined();
    //             let callback = js_callback.into_inner(&mut task_context);
    //
    //             let callback_arguments = match result {
    //                 Ok(proof) => {
    //                     let js_buffer = JsBuffer::external(&mut task_context, proof.clone());
    //                     let js_value = js_buffer.as_value(&mut task_context);
    //
    //                     vec![task_context.null().upcast(), js_value.upcast()]
    //                 }
    //
    //                 // Convert the error to a JavaScript exception on failure
    //                 Err(err) => vec![task_context.error(err.to_string())?.upcast()],
    //             };
    //
    //             callback.call(&mut task_context, this, callback_arguments)?;
    //
    //             Ok(())
    //         });
    //     })
    //     .or_else(|err| cx.throw_error(err.to_string()))?;
    //
    //     // The result is returned through the callback, not through direct return
    //     Ok(cx.undefined())
    // }
    //
    // /// Flush data on disc and then calls js callback passed as a first
    // /// argument to the function
    // fn js_grove_db_flush(mut cx: FunctionContext) -> JsResult<JsUndefined> {
    //     let js_callback = cx.argument::<JsFunction>(0)?.root(&mut cx);
    //
    //     let db = cx
    //         .this()
    //         .downcast_or_throw::<JsBox<PlatformWrapper>, _>(&mut cx)?;
    //
    //     db.flush(|channel| {
    //         channel.send(move |mut task_context| {
    //             let callback = js_callback.into_inner(&mut task_context);
    //             let this = task_context.undefined();
    //             let callback_arguments: Vec<Handle<JsValue>> = vec![task_context.null().upcast()];
    //
    //             callback.call(&mut task_context, this, callback_arguments)?;
    //
    //             Ok(())
    //         });
    //     })
    //     .or_else(|err| cx.throw_error(err.to_string()))?;
    //
    //     Ok(cx.undefined())
    // }
    //
    // /// Returns root hash or empty buffer
    // fn js_grove_db_root_hash(mut cx: FunctionContext) -> JsResult<JsUndefined> {
    //     let js_transaction = cx.argument::<JsValue>(0)?;
    //
    //     let maybe_transaction = if js_transaction.is_a::<JsUndefined, _>(&mut cx) {
    //         None
    //     } else {
    //         Some(
    //             js_transaction
    //                 .downcast_or_throw::<JsBox<PlatformWrapperTransaction>, _>(&mut cx)?
    //                 .deref()
    //                 .deref()
    //                 .clone(),
    //         )
    //     };
    //
    //     let js_callback = cx.argument::<JsFunction>(1)?.root(&mut cx);
    //
    //     let db = cx
    //         .this()
    //         .downcast_or_throw::<JsBox<PlatformWrapper>, _>(&mut cx)?;
    //
    //     db.send_to_drive_thread(move |platform: &Platform, channel| {
    //         let grove_db = &platform.drive.grove;
    //
    //         let result = grove_db
    //             .root_hash(maybe_transaction.map(|wrapper| wrapper.deref()))
    //             .unwrap();
    //
    //         channel.send(move |mut task_context| {
    //             let callback = js_callback.into_inner(&mut task_context);
    //             let this = task_context.undefined();
    //
    //             let callback_arguments: Vec<Handle<JsValue>> = match result {
    //                 Ok(hash) => vec![
    //                     task_context.null().upcast(),
    //                     JsBuffer::external(&mut task_context, hash).upcast(),
    //                 ],
    //                 Err(err) => vec![task_context.error(err.to_string())?.upcast()],
    //             };
    //
    //             callback.call(&mut task_context, this, callback_arguments)?;
    //
    //             Ok(())
    //         });
    //     })
    //     .or_else(|err| cx.throw_error(err.to_string()))?;
    //
    //     // The result is returned through the callback, not through direct return
    //     Ok(cx.undefined())
    // }
    //
    // fn js_grove_db_delete(mut cx: FunctionContext) -> JsResult<JsUndefined> {
    //     let js_path = cx.argument::<JsArray>(0)?;
    //     let js_key = cx.argument::<JsBuffer>(1)?;
    //
    //     let js_transaction = cx.argument::<JsValue>(2)?;
    //
    //     let maybe_transaction = if js_transaction.is_a::<JsUndefined, _>(&mut cx) {
    //         None
    //     } else {
    //         Some(
    //             js_transaction
    //                 .downcast_or_throw::<JsBox<PlatformWrapperTransaction>, _>(&mut cx)?
    //                 .deref()
    //                 .deref()
    //                 .clone(),
    //         )
    //     };
    //
    //     let js_callback = cx.argument::<JsFunction>(3)?.root(&mut cx);
    //
    //     let path = converter::js_array_of_buffers_to_vec(js_path, &mut cx)?;
    //     let key = converter::js_buffer_to_vec_u8(js_key, &mut cx);
    //
    //     let db = cx
    //         .this()
    //         .downcast_or_throw::<JsBox<PlatformWrapper>, _>(&mut cx)?;
    //
    //     db.send_to_drive_thread(move |platform: &Platform, channel| {
    //         let grove_db = &platform.drive.grove;
    //
    //         let path_slice: Vec<&[u8]> = path.iter().map(|fragment| fragment.as_slice()).collect();
    //         let result = grove_db
    //             .delete(
    //                 path_slice,
    //                 key.as_slice(),
    //                 maybe_transaction.map(|wrapper| wrapper.deref()),
    //             )
    //             .unwrap();
    //
    //         channel.send(move |mut task_context| {
    //             let callback = js_callback.into_inner(&mut task_context);
    //             let this = task_context.undefined();
    //             let callback_arguments: Vec<Handle<JsValue>> = match result {
    //                 Ok(()) => {
    //                     vec![task_context.null().upcast()]
    //                 }
    //
    //                 // Convert the error to a JavaScript exception on failure
    //                 Err(err) => vec![task_context.error(err.to_string())?.upcast()],
    //             };
    //
    //             callback.call(&mut task_context, this, callback_arguments)?;
    //
    //             Ok(())
    //         });
    //     })
    //     .or_else(|err| cx.throw_error(err.to_string()))?;
    //
    //     // The result is returned through the callback, not through direct return
    //     Ok(cx.undefined())
    // }
    //
    // fn js_abci_init_chain(mut cx: FunctionContext) -> JsResult<JsUndefined> {
    //     let js_request = cx.argument::<JsBuffer>(0)?;
    //     let js_transaction = cx.argument::<JsValue>(1)?;
    //
    //     let maybe_transaction = if js_transaction.is_a::<JsUndefined, _>(&mut cx) {
    //         None
    //     } else {
    //         Some(
    //             js_transaction
    //                 .downcast_or_throw::<JsBox<PlatformWrapperTransaction>, _>(&mut cx)?
    //                 .deref()
    //                 .deref()
    //                 .clone(),
    //         )
    //     };
    //
    //     let js_callback = cx.argument::<JsFunction>(2)?.root(&mut cx);
    //
    //     let db = cx
    //         .this()
    //         .downcast_or_throw::<JsBox<PlatformWrapper>, _>(&mut cx)?;
    //
    //     let request_bytes = converter::js_buffer_to_vec_u8(js_request, &mut cx);
    //
    //     db.send_to_drive_thread(move |platform: &Platform, channel| {
    //         let result = InitChainRequest::from_bytes(&request_bytes)
    //             .and_then(|request| {
    //                 platform.init_chain(request, maybe_transaction.map(|wrapper| wrapper.deref()))
    //             })
    //             .and_then(|response| response.to_bytes());
    //
    //         channel.send(move |mut task_context| {
    //             let callback = js_callback.into_inner(&mut task_context);
    //             let this = task_context.undefined();
    //
    //             let callback_arguments: Vec<Handle<JsValue>> = match result {
    //                 Ok(response_bytes) => {
    //                     let value = JsBuffer::external(&mut task_context, response_bytes);
    //
    //                     vec![task_context.null().upcast(), value.upcast()]
    //                 }
    //
    //                 // Convert the error to a JavaScript exception on failure
    //                 Err(err) => vec![task_context.error(err.to_string())?.upcast()],
    //             };
    //
    //             callback.call(&mut task_context, this, callback_arguments)?;
    //
    //             Ok(())
    //         });
    //     })
    //     .or_else(|err| cx.throw_error(err.to_string()))?;
    //
    //     // The result is returned through the callback, not through direct return
    //     Ok(cx.undefined())
    // }
    //
    // fn js_abci_block_begin(mut cx: FunctionContext) -> JsResult<JsUndefined> {
    //     let js_request = cx.argument::<JsBuffer>(0)?;
    //     let js_transaction = cx.argument::<JsValue>(1)?;
    //
    //     let maybe_transaction = if js_transaction.is_a::<JsUndefined, _>(&mut cx) {
    //         None
    //     } else {
    //         Some(
    //             js_transaction
    //                 .downcast_or_throw::<JsBox<PlatformWrapperTransaction>, _>(&mut cx)?
    //                 .deref()
    //                 .deref()
    //                 .clone(),
    //         )
    //     };
    //
    //     let js_callback = cx.argument::<JsFunction>(2)?.root(&mut cx);
    //
    //     let db = cx
    //         .this()
    //         .downcast_or_throw::<JsBox<PlatformWrapper>, _>(&mut cx)?;
    //
    //     let request_bytes = converter::js_buffer_to_vec_u8(js_request, &mut cx);
    //
    //     db.send_to_drive_thread(move |platform: &Platform, channel| {
    //         let result = BlockBeginRequest::from_bytes(&request_bytes)
    //             .and_then(|request| {
    //                 platform.block_begin(request, maybe_transaction.map(|wrapper| wrapper.deref()))
    //             })
    //             .and_then(|response| response.to_bytes());
    //
    //         channel.send(move |mut task_context| {
    //             let callback = js_callback.into_inner(&mut task_context);
    //             let this = task_context.undefined();
    //
    //             let callback_arguments: Vec<Handle<JsValue>> = match result {
    //                 Ok(response_bytes) => {
    //                     let value = JsBuffer::external(&mut task_context, response_bytes);
    //
    //                     vec![task_context.null().upcast(), value.upcast()]
    //                 }
    //
    //                 // Convert the error to a JavaScript exception on failure
    //                 Err(err) => vec![task_context.error(err.to_string())?.upcast()],
    //             };
    //
    //             callback.call(&mut task_context, this, callback_arguments)?;
    //
    //             Ok(())
    //         });
    //     })
    //     .or_else(|err| cx.throw_error(err.to_string()))?;
    //
    //     // The result is returned through the callback, not through direct return
    //     Ok(cx.undefined())
    // }
    //
    // fn js_abci_block_end(mut cx: FunctionContext) -> JsResult<JsUndefined> {
    //     let js_request = cx.argument::<JsBuffer>(0)?;
    //     let js_transaction = cx.argument::<JsValue>(1)?;
    //
    //     let maybe_transaction = if js_transaction.is_a::<JsUndefined, _>(&mut cx) {
    //         None
    //     } else {
    //         Some(
    //             js_transaction
    //                 .downcast_or_throw::<JsBox<PlatformWrapperTransaction>, _>(&mut cx)?
    //                 .deref()
    //                 .deref()
    //                 .clone(),
    //         )
    //     };
    //
    //     let js_callback = cx.argument::<JsFunction>(2)?.root(&mut cx);
    //
    //     let db = cx
    //         .this()
    //         .downcast_or_throw::<JsBox<PlatformWrapper>, _>(&mut cx)?;
    //
    //     let request_bytes = converter::js_buffer_to_vec_u8(js_request, &mut cx);
    //
    //     db.send_to_drive_thread(move |platform: &Platform, channel| {
    //         let result = BlockEndRequest::from_bytes(&request_bytes)
    //             .and_then(|request| {
    //                 platform.block_end(request, maybe_transaction.map(|wrapper| wrapper.deref()))
    //             })
    //             .and_then(|response| response.to_bytes());
    //
    //         channel.send(move |mut task_context| {
    //             let callback = js_callback.into_inner(&mut task_context);
    //             let this = task_context.undefined();
    //
    //             let callback_arguments: Vec<Handle<JsValue>> = match result {
    //                 Ok(response_bytes) => {
    //                     let value = JsBuffer::external(&mut task_context, response_bytes);
    //
    //                     vec![task_context.null().upcast(), value.upcast()]
    //                 }
    //
    //                 // Convert the error to a JavaScript exception on failure
    //                 Err(err) => vec![task_context.error(err.to_string())?.upcast()],
    //             };
    //
    //             callback.call(&mut task_context, this, callback_arguments)?;
    //
    //             Ok(())
    //         });
    //     })
    //     .or_else(|err| cx.throw_error(err.to_string()))?;
    //
    //     // The result is returned through the callback, not through direct return
    //     Ok(cx.undefined())
    // }
}

// #[neon::main]
// fn main(mut cx: ModuleContext) -> NeonResult<()> {
//     cx.export_function("driveOpen", PlatformWrapper::js_open)?;
//     cx.export_function("driveClose", PlatformWrapper::js_close)?;
//     cx.export_function(
//         "driveCreateInitialStateStructure",
//         PlatformWrapper::js_create_initial_state_structure,
//     )?;
//     cx.export_function("driveApplyContract", PlatformWrapper::js_apply_contract)?;
//     cx.export_function(
//         "driveCreateDocument",
//         PlatformWrapper::js_add_document_for_contract_cbor,
//     )?;
//     cx.export_function(
//         "driveUpdateDocument",
//         PlatformWrapper::js_update_document_for_contract_cbor,
//     )?;
//     cx.export_function(
//         "driveDeleteDocument",
//         PlatformWrapper::js_delete_document_for_contract_cbor,
//     )?;
//     cx.export_function(
//         "driveInsertIdentity",
//         PlatformWrapper::js_insert_identity_cbor,
//     )?;
//     cx.export_function("driveQueryDocuments", PlatformWrapper::js_query_documents)?;
//
//     cx.export_function(
//         "driveProveDocumentsQuery",
//         PlatformWrapper::js_prove_documents_query,
//     )?;
//
//     cx.export_function("groveDbInsert", PlatformWrapper::js_grove_db_insert)?;
//     cx.export_function(
//         "groveDbInsertIfNotExists",
//         PlatformWrapper::js_grove_db_insert_if_not_exists,
//     )?;
//     cx.export_function("groveDbGet", PlatformWrapper::js_grove_db_get)?;
//     cx.export_function("groveDbDelete", PlatformWrapper::js_grove_db_delete)?;
//     cx.export_function("groveDbFlush", PlatformWrapper::js_grove_db_flush)?;
//     cx.export_function(
//         "groveDbStartTransaction",
//         PlatformWrapper::js_grove_db_start_transaction,
//     )?;
//     cx.export_function(
//         "groveDbCommitTransaction",
//         PlatformWrapper::js_grove_db_commit_transaction,
//     )?;
//     cx.export_function(
//         "groveDbRollbackTransaction",
//         PlatformWrapper::js_grove_db_rollback_transaction,
//     )?;
//     cx.export_function(
//         "groveDbAbortTransaction",
//         PlatformWrapper::js_grove_db_abort_transaction,
//     )?;
//     cx.export_function("groveDbPutAux", PlatformWrapper::js_grove_db_put_aux)?;
//     cx.export_function("groveDbDeleteAux", PlatformWrapper::js_grove_db_delete_aux)?;
//     cx.export_function("groveDbGetAux", PlatformWrapper::js_grove_db_get_aux)?;
//     cx.export_function("groveDbQuery", PlatformWrapper::js_grove_db_query)?;
//     cx.export_function(
//         "groveDbProveQuery",
//         PlatformWrapper::js_grove_db_prove_query,
//     )?;
//     cx.export_function(
//         "groveDbProveQueryMany",
//         PlatformWrapper::js_grove_db_prove_query_many,
//     )?;
//     cx.export_function("groveDbRootHash", PlatformWrapper::js_grove_db_root_hash)?;
//
//     cx.export_function("abciInitChain", PlatformWrapper::js_abci_init_chain)?;
//     cx.export_function("abciBlockBegin", PlatformWrapper::js_abci_block_begin)?;
//     cx.export_function("abciBlockEnd", PlatformWrapper::js_abci_block_end)?;
//
//     Ok(())
// }
