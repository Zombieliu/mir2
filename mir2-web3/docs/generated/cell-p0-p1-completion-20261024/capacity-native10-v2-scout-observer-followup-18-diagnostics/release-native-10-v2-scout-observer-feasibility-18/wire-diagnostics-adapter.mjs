import {pathToFileURL}from'node:url';
import {verifyWireDiagnosticSource,CORE_CODE_EVIDENCE_COMMIT,CURRENT_DRIVER_COMMIT,TERMINAL_SCOPE}from'./wire-diagnostics-contract.mjs';

// The caller supplies the original run function/options and public persistence.
// No writer.close hook, account preparation, network or realm creation occurs here.
export async function runCapacityWithWireDiagnostics({CapacityClient,runCapacity,options,writeTerminal}){
 if(typeof runCapacity!=='function'||typeof writeTerminal!=='function')throw TypeError('Original run function and terminal persistence are required');
 const source=await verifyWireDiagnosticSource();
 const{installCompactNavigationDiagnostics}=await import(pathToFileURL(source.wrapper.file));
 const controller=installCompactNavigationDiagnostics(CapacityClient,{emit:(row,writer)=>writer.add(row)});
 let report,originalError,originalThrew=false,diagnosticError,statusBeforeUninstall=null,statusAfterUninstall=null,statusCaptureFailed=false,uninstallSucceeded=false,uninstallFailed=false;
 try{report=await runCapacity(options);}catch(error){originalThrew=true;originalError=error;}
 finally{
  try{statusBeforeUninstall=controller.status();}catch(error){statusCaptureFailed=true;diagnosticError??=error;}
  try{controller.uninstall();uninstallSucceeded=true;}catch(error){uninstallFailed=true;diagnosticError??=error;}
  try{statusAfterUninstall=controller.status();}catch(error){statusCaptureFailed=true;diagnosticError??=error;}
  const terminal={schemaVersion:1,diagnosticVersion:'v2',capturedAtUtc:new Date().toISOString(),driverSourceCommit:CURRENT_DRIVER_COMMIT,coreCodeEvidenceCommit:CORE_CODE_EVIDENCE_COMMIT,coreDriverSourceSha256:source.coreDriverSourceSha256,wrapperManifest:source.manifest,wrapperReview:source.review,wrapperSourceSha256:source.sourceSha256,statusCapturePhase:'before-uninstall-after-runCapacity-settled',statusBeforeUninstall,statusAfterUninstall,statusCaptureFailed,uninstallSucceeded,uninstallFailed,runReturned:!originalThrew,runThrew:originalThrew,runId:typeof report?.runId==='string'&&/^\d+-[a-f0-9]{8}$/.test(report.runId)?report.runId:null,terminalOutsideRaw:true,callback:'existing-bounded-capacity-writer',existingWriterMaxBufferedBytes:4194304,emitReturnCountIsDurable:false,scope:TERMINAL_SCOPE};
  try{await writeTerminal(terminal);}catch(error){diagnosticError??=error;}
 }
 if(originalThrew)throw originalError; // Never replace the original error object.
 if(diagnosticError)throw diagnosticError;
 return report;
}
