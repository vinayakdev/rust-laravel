<?php

namespace Acme\LocalPackage\Models;

use Illuminate\Database\Eloquent\Model;

class FeatureFlag extends Model
{
    protected $table = 'feature_flags';

    protected $fillable = [
        'key',
        'description',
        'enabled',
        'audience',
    ];

    protected $casts = [
        'enabled' => 'boolean',
        'audience' => 'array',
    ];
}
