const normalize = value => String(value ?? '').replace(/[^a-z0-9]/gi, '').toLowerCase();

function questEntry(snapshot, questId) {
  return (snapshot?.questLog ?? []).find(entry => Number(entry?.questId) === questId) ?? null;
}

function flagObjectiveEntry(snapshot, questId, flag, flagIndex, flagCount) {
  const objectives = questEntry(snapshot, questId)?.objectives ?? [];
  const wanted = normalize(flag?.message);
  const byLabel = objectives.find(entry => {
    const label = normalize(entry?.label);
    return wanted && label && (label.includes(wanted) || wanted.includes(label));
  });
  return byLabel ?? objectives[Math.max(0, objectives.length - flagCount + flagIndex)] ?? null;
}

function objectiveDone(entry) {
  return entry?.done === true ||
    (Number(entry?.required) > 0 && Number(entry?.current) >= Number(entry.required));
}

function objectiveAdvanced(before, after) {
  return objectiveDone(after) || Number(after?.current ?? 0) > Number(before?.current ?? 0);
}

function sameNpc(entity, metadata) {
  if (!entity || String(entity.kind ?? '').toLowerCase() !== 'npc') return false;
  if (metadata?.objectId != null && String(entity.objectId) === String(metadata.objectId)) return true;
  const point = metadata?.position ?? metadata;
  return normalize(entity.name) === normalize(metadata?.name) &&
    Number(entity.x) === Number(point?.x) && Number(entity.y) === Number(point?.y);
}

function liveNpc(snapshot, metadata) {
  return (snapshot?.entities ?? []).find(entity => sameNpc(entity, metadata)) ?? null;
}

function actualDialogLink(snapshot, expectedTarget) {
  const wanted = String(expectedTarget ?? '').trim().toLowerCase();
  return (snapshot?.activeNpcDialog?.links ?? []).find(link =>
    String(link?.target ?? '').trim().toLowerCase() === wanted
  ) ?? null;
}

async function attemptSetter(client, questId, flag, flagIndex, flagCount, setter, navigateNear, travelToMap) {
  const metadata = setter?.npc;
  if (!metadata?.mapFileName || !metadata?.position) return false;
  if (String(client.snapshot?.mapFileName) !== String(metadata.mapFileName)) {
    await travelToMap(metadata.mapFileName);
  }
  await navigateNear(metadata.position, 1);
  const npc = await client.wait(
    () => liveNpc(client.snapshot, metadata),
    `q${questId} flag ${flag.number} live setter`,
    20_000,
  );
  const before = flagObjectiveEntry(client.snapshot, questId, flag, flagIndex, flagCount);
  const afterInteractSequence = client.sequence;
  client.send({ type: 'interact', objectId: Number(npc.objectId) });
  await client.wait(
    () => {
      const current = flagObjectiveEntry(client.snapshot, questId, flag, flagIndex, flagCount);
      return client.sequence > afterInteractSequence &&
        (objectiveAdvanced(before, current) || client.snapshot?.activeNpcDialog != null);
    },
    `q${questId} flag ${flag.number} setter response`,
    20_000,
  );
  if (objectiveAdvanced(before, flagObjectiveEntry(client.snapshot, questId, flag, flagIndex, flagCount))) {
    return true;
  }

  // The static route identifies which choices are expected, but never
  // authorizes them. Resolve every choice from the live dialog before sending.
  for (const expectedTarget of setter.targetSequence ?? []) {
    const link = actualDialogLink(client.snapshot, expectedTarget);
    if (!link) return false;
    const beforeChoiceSequence = client.sequence;
    client.send({ type: 'selectNpcDialog', target: link.target });
    await client.wait(
      () => {
        const current = flagObjectiveEntry(client.snapshot, questId, flag, flagIndex, flagCount);
        return client.sequence > beforeChoiceSequence &&
          (objectiveAdvanced(before, current) || client.snapshot?.activeNpcDialog != null);
      },
      `q${questId} flag ${flag.number} dialog target ${link.target}`,
      20_000,
    );
    if (objectiveAdvanced(before, flagObjectiveEntry(client.snapshot, questId, flag, flagIndex, flagCount))) {
      return true;
    }
  }
  return client.wait(
    () => objectiveAdvanced(
      before,
      flagObjectiveEntry(client.snapshot, questId, flag, flagIndex, flagCount),
    ),
    `q${questId} flag ${flag.number} objective progress`,
    20_000,
  ).then(() => true).catch(() => false);
}

/** Complete data-driven flag objectives through ordinary travel and live NPC interaction. */
export async function completeFlagObjectives(client, routeQuest, navigateNear, travelToMap) {
  const questId = Number(routeQuest?.questId);
  const flags = routeQuest?.objectives?.flag ?? [];
  if (!Number.isSafeInteger(questId) || questId <= 0) throw new Error('routeQuest.questId must be a positive integer');
  if (!Array.isArray(flags) || flags.length === 0) return [];
  if (typeof navigateNear !== 'function' || typeof travelToMap !== 'function') {
    throw new Error('navigateNear and travelToMap are required');
  }

  const completed = [];
  for (let flagIndex = 0; flagIndex < flags.length; flagIndex += 1) {
    const flag = flags[flagIndex];
    if (objectiveDone(flagObjectiveEntry(client.snapshot, questId, flag, flagIndex, flags.length))) {
      completed.push(Number(flag.number));
      continue;
    }
    const setters = [...(flag.setters ?? [])].sort((left, right) =>
      Number(String(right?.npc?.mapFileName) === String(client.snapshot?.mapFileName)) -
      Number(String(left?.npc?.mapFileName) === String(client.snapshot?.mapFileName)) ||
      Number(left?.targetSequence?.length ?? 0) - Number(right?.targetSequence?.length ?? 0)
    );
    if (!setters.length) throw new Error(`q${questId} flag ${flag.number} has no setter metadata`);
    let advanced = false;
    for (const setter of setters) {
      if (await attemptSetter(client, questId, flag, flagIndex, flags.length, setter, navigateNear, travelToMap)) {
        advanced = true;
        break;
      }
    }
    if (!advanced) throw new Error(`q${questId} flag ${flag.number} did not advance through a live setter`);
    completed.push(Number(flag.number));
  }
  return completed;
}
