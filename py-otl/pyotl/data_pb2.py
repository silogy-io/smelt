# -*- coding: utf-8 -*-
# Generated by the protocol buffer compiler.  DO NOT EDIT!
# source: data.proto
# Protobuf Python Version: 5.26.1
"""Generated protocol buffer code."""
from google.protobuf import descriptor as _descriptor
from google.protobuf import descriptor_pool as _descriptor_pool
from google.protobuf import symbol_database as _symbol_database
from google.protobuf.internal import builder as _builder
# @@protoc_insertion_point(imports)

_sym_db = _symbol_database.Default()


from google.protobuf import duration_pb2 as google_dot_protobuf_dot_duration__pb2
from google.protobuf import timestamp_pb2 as google_dot_protobuf_dot_timestamp__pb2


DESCRIPTOR = _descriptor_pool.Default().AddSerializedFile(b'\n\ndata.proto\x12\x12otl_telemetry.data\x1a\x1egoogle/protobuf/duration.proto\x1a\x1fgoogle/protobuf/timestamp.proto\"\xb1\x01\n\x05\x45vent\x12(\n\x04time\x18\x01 \x01(\x0b\x32\x1a.google.protobuf.Timestamp\x12\x10\n\x08trace_id\x18\x02 \x01(\t\x12\x33\n\x07\x63ommand\x18\x0f \x01(\x0b\x32 .otl_telemetry.data.CommandEventH\x00\x12\x31\n\x06invoke\x18\x10 \x01(\x0b\x32\x1f.otl_telemetry.data.InvokeEventH\x00\x42\x04\n\x02\x65t\"\x9b\x02\n\x0c\x43ommandEvent\x12\x13\n\x0b\x63ommand_ref\x18\x01 \x01(\t\x12\x39\n\tscheduled\x18\x04 \x01(\x0b\x32$.otl_telemetry.data.CommandScheduledH\x00\x12\x35\n\x07started\x18\x05 \x01(\x0b\x32\".otl_telemetry.data.CommandStartedH\x00\x12\x39\n\tcancelled\x18\x06 \x01(\x0b\x32$.otl_telemetry.data.CommandCancelledH\x00\x12\x37\n\x08\x66inished\x18\x07 \x01(\x0b\x32#.otl_telemetry.data.CommandFinishedH\x00\x42\x10\n\x0e\x43ommandVariant\"\x12\n\x10\x43ommandScheduled\"\x10\n\x0e\x43ommandStarted\"\x12\n\x10\x43ommandCancelled\"A\n\x0f\x43ommandFinished\x12.\n\x03out\x18\x01 \x01(\x0b\x32!.otl_telemetry.data.CommandOutput\"$\n\rCommandOutput\x12\x13\n\x0bstatus_code\x18\x01 \x01(\x05\"\x88\x01\n\x0bInvokeEvent\x12\x33\n\x05start\x18\x05 \x01(\x0b\x32\".otl_telemetry.data.ExecutionStartH\x00\x12\x33\n\x04\x64one\x18\x06 \x01(\x0b\x32#.otl_telemetry.data.AllCommandsDoneH\x00\x42\x0f\n\rInvokeVariant\"B\n\x0e\x45xecutionStart\x12\x0c\n\x04path\x18\x01 \x01(\t\x12\x10\n\x08username\x18\x02 \x01(\t\x12\x10\n\x08hostname\x18\x03 \x01(\t\"\x11\n\x0f\x41llCommandsDoneb\x06proto3')

_globals = globals()
_builder.BuildMessageAndEnumDescriptors(DESCRIPTOR, _globals)
_builder.BuildTopDescriptorsAndMessages(DESCRIPTOR, 'data_pb2', _globals)
if not _descriptor._USE_C_DESCRIPTORS:
  DESCRIPTOR._loaded_options = None
  _globals['_EVENT']._serialized_start=100
  _globals['_EVENT']._serialized_end=277
  _globals['_COMMANDEVENT']._serialized_start=280
  _globals['_COMMANDEVENT']._serialized_end=563
  _globals['_COMMANDSCHEDULED']._serialized_start=565
  _globals['_COMMANDSCHEDULED']._serialized_end=583
  _globals['_COMMANDSTARTED']._serialized_start=585
  _globals['_COMMANDSTARTED']._serialized_end=601
  _globals['_COMMANDCANCELLED']._serialized_start=603
  _globals['_COMMANDCANCELLED']._serialized_end=621
  _globals['_COMMANDFINISHED']._serialized_start=623
  _globals['_COMMANDFINISHED']._serialized_end=688
  _globals['_COMMANDOUTPUT']._serialized_start=690
  _globals['_COMMANDOUTPUT']._serialized_end=726
  _globals['_INVOKEEVENT']._serialized_start=729
  _globals['_INVOKEEVENT']._serialized_end=865
  _globals['_EXECUTIONSTART']._serialized_start=867
  _globals['_EXECUTIONSTART']._serialized_end=933
  _globals['_ALLCOMMANDSDONE']._serialized_start=935
  _globals['_ALLCOMMANDSDONE']._serialized_end=952
# @@protoc_insertion_point(module_scope)