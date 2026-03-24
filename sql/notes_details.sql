select 
notes.note_id as id,
any_value(notes.note_title) as title,
any_value(notes.modified) as modified,
any_value(notes.note_type) as type,
any_value(notes.path) as path,
list(distinct tags.tag) as tags,
list(distinct author_notes.note_title) as authors,
from notes
left join tags on notes.note_id = tags.note_id
left join (select to_anchor from refs where kind == 'author') as authors on notes.note_id = authors.source
left join notes as author_notes on authors.to_anchor = author_notes.note_id
left join refs on notes.note_id = refs.note_id
group by notes.note_id
order by modified desc
