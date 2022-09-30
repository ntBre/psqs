#!/usr/bin/awk -f

{
    printf "Atom::new_from_label(\"%s\", %.10f, %.10f, %.10f),\n",
	$1, $2, $3, $4
}
