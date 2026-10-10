SELECT COUNT(*) FILTER (WHERE visits>0), COUNT(*) FILTER (WHERE visits>0 AND first_visit>=$1),
 COUNT(*) FILTER (WHERE visits>0 AND first_visit<$1), COUNT(*) FILTER (WHERE visits>=3),
 COUNT(*) FILTER (WHERE date_diff('day',last_visit,$2)>=30 AND date_diff('day',last_visit,$2)<90),
 COUNT(*) FILTER (WHERE prior>0),COUNT(*) FILTER (WHERE prior>0 AND visits>0) FROM visits
