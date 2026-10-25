export function assertMeasuredPhase(stage,nominalSeconds,requireComplete=false){
 if(!stage||!Number.isFinite(nominalSeconds)||nominalSeconds<=0||stage.seconds!==nominalSeconds||!Number.isSafeInteger(stage.startedAtMs)||!Number.isSafeInteger(stage.endedAtMs)||stage.startedAtMs<0||stage.endedAtMs<stage.startedAtMs||!Number.isFinite(stage.measuredSeconds)||Object.is(stage.measuredSeconds,-0)||stage.measuredSeconds!==(stage.endedAtMs-stage.startedAtMs)/1000)throw Error('Measured phase seconds must preserve the actual finite wall interval and configured duration');
 if((requireComplete||stage.completed===true)&&stage.measuredSeconds<nominalSeconds)throw Error('Completed measured phase is shorter than its original minimum duration');
 return stage.measuredSeconds;
}
