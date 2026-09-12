export function visibleDuplicateSets(sets, hiddenPaths) {
  if (!sets?.length) return [];
  if (!hiddenPaths?.size) return sets;

  return sets
    .map((set) => {
      const paths = set.paths.filter((path) => !hiddenPaths.has(path));
      return {
        ...set,
        paths,
        wasted: set.size * Math.max(0, paths.length - 1),
      };
    })
    .filter((set) => set.paths.length > 1);
}
