-- WAL-only workers used segment numbers as manifest revision names. Reserve
-- that entire namespace before snapshot revisions are added to an existing cell.
UPDATE replication_generations g SET manifest_revision=COALESCE(
  (SELECT MAX(segment_number) FROM replication_segments s WHERE s.generation_id=g.id),0
) WHERE g.manifest_revision=0;
