const SHA = /^[a-f0-9]{40}$/;
const validFile = file => typeof file?.filename === 'string' && file.filename.length > 0
  && SHA.test(file.sha) && typeof file.status === 'string';
const sameFiles = (actual, expected) => {
  if (!Array.isArray(actual) || !Array.isArray(expected) || actual.length !== expected.length
      || !actual.every(validFile) || !expected.every(validFile)) return false;
  const byPath = new Map(actual.map(file => [file.filename, file]));
  return byPath.size === actual.length && new Set(expected.map(file => file.filename)).size === expected.length
    && expected.every(file => byPath.get(file.filename)?.status === file.status && byPath.get(file.filename)?.sha === file.sha);
};

/** Verify every bounded comparison in a reviewed, contiguous source chain.
 * Checkpoint sources are provenance anchors, not claims about deployed traffic.
 * Multi-segment composition deliberately supports only added/modified paths;
 * a removal/rename needs a reviewed extension, rather than an approximate union. */
export async function verifyNumeronSourceClosure(release, source, readComparison, requireRelease) {
  const check = condition => requireRelease(condition, 'qrSourceClosureMismatch');
  check(SHA.test(source) && source === release.source && SHA.test(release.baseSource));
  const segmented = release.sourceCheckpoints !== undefined;
  const segments = segmented ? release.sourceCheckpoints : [{
    baseSource: release.baseSource, source, sourceCommits: release.sourceCommits, changedFiles: release.changedFiles,
  }];
  check(Array.isArray(segments) && segments.length >= (segmented ? 2 : 1) && segments.length <= 8);
  let previous = release.baseSource;
  const allCommits = [], aggregate = new Map(), verified = [];
  for (const segment of segments) {
    check(SHA.test(segment?.baseSource) && segment.baseSource === previous && SHA.test(segment.source)
      && segment.source !== segment.baseSource && Array.isArray(segment.sourceCommits) && segment.sourceCommits.length > 0
      && segment.sourceCommits.every(commit => SHA.test(commit)) && Array.isArray(segment.changedFiles)
      && segment.changedFiles.length < 300 && segment.changedFiles.every(validFile));
    const expected = new Set(segment.sourceCommits);
    check(expected.size === segment.sourceCommits.length && expected.has(segment.source) && !expected.has(segment.baseSource));
    const comparison = await readComparison(segment.baseSource, segment.source);
    const commits = Array.isArray(comparison?.commits) ? comparison.commits : [];
    const actual = new Set(commits.map(commit => commit?.sha));
    check(comparison?.status === 'ahead' && comparison.ahead_by === expected.size && comparison.behind_by === 0
      && comparison.merge_base_commit?.sha === segment.baseSource && commits.length === expected.size
      && actual.size === expected.size && [...actual].every(commit => expected.has(commit))
      && sameFiles(comparison.files, segment.changedFiles));
    if (segmented) {
      for (const file of segment.changedFiles) {
        const existing = aggregate.get(file.filename);
        check(['added', 'modified'].includes(file.status) && !(existing && file.status === 'added'));
        aggregate.set(file.filename, { filename: file.filename, sha: file.sha,
          status: existing?.status === 'added' ? 'added' : file.status });
      }
    }
    allCommits.push(...segment.sourceCommits);
    verified.push({ base: segment.baseSource, source: segment.source, commits: segment.sourceCommits.length,
      files: segment.changedFiles.length });
    previous = segment.source;
  }
  check(previous === source && Array.isArray(release.sourceCommits)
    && new Set(allCommits).size === allCommits.length && allCommits.length === release.sourceCommits.length
    && allCommits.every((commit, index) => commit === release.sourceCommits[index]));
  if (segmented) check(sameFiles([...aggregate.values()], release.changedFiles));
  return { base: release.baseSource, commits: release.sourceCommits, changedFiles: release.changedFiles,
    ...(segmented ? { checkpoints: verified } : {}) };
}
