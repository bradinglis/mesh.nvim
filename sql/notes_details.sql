select 
notes.note_id as id,
any_value(notes.note_title) as title,
any_value(notes.modified) as modified,
any_value(notes.note_type) as type,
any_value(notes.path) as path,
list(distinct tags.tag) as tags,
list(distinct author_notes.note_title) as authors,
list(distinct parents.parent) as parents,
from notes
left join tags on notes.note_id = tags.note_id
left join authors on notes.note_id = authors.source
left join notes as author_notes on authors.author = author_notes.note_id
left join refs on notes.note_id = refs.note_id
left join parents on notes.note_id = parents.child
group by notes.note_id
order by modified desc
